use serde::{Deserialize,Serialize};
use std::{collections::HashMap,fs,path::{Path,PathBuf},process::Stdio,sync::{Arc,Mutex},sync::atomic::{AtomicU64,Ordering}};
use tauri::{AppHandle,Emitter,Manager,State};
use tokio::{io::{AsyncBufReadExt,BufReader},process::{Child,Command},sync::{mpsc,Mutex as AsyncMutex}};

#[derive(Default,Clone)]
struct AppState{roots:Arc<Mutex<Vec<PathBuf>>>,downloads:Arc<DownloadStore>}

struct DownloadStore{next_id:AtomicU64,jobs:AsyncMutex<HashMap<u64,DownloadJob>>}
impl Default for DownloadStore{fn default()->Self{Self{next_id:AtomicU64::new(1),jobs:AsyncMutex::new(HashMap::new())}}}

#[derive(Clone)]
struct DownloadJob{request:DownloadRequest,control:mpsc::Sender<Control>,status:String}

#[derive(Clone,Deserialize)]
struct DownloadRequest{url:String,destination:PathBuf,filename:String,format:String,quality:String,#[serde(default)]job_id:Option<u64>,#[serde(default)]resume:bool}

#[derive(Clone,Serialize)]
struct DownloadEvent{id:u64,status:String,percent:f64,speed:String,eta:String,filename:String,message:Option<String>}

#[derive(Serialize,Clone)]
struct RootView{id:String,name:String,path:String}
#[derive(Serialize,Clone)]
struct FileEntry{name:String,path:String,is_dir:bool,size:u64,extension:Option<String>}
#[derive(Serialize)]
struct MediaInfo{title:String,thumbnail:Option<String>,duration:Option<String>,uploader:Option<String>}

fn config_file(app:&AppHandle)->Result<PathBuf,String>{let dir=app.path().app_config_dir().map_err(|e|e.to_string())?;fs::create_dir_all(&dir).map_err(|e|e.to_string())?;Ok(dir.join("roots.json"))}
fn load_roots(app:&AppHandle)->Vec<PathBuf>{let Ok(path)=config_file(app)else{return vec![]};let Ok(raw)=fs::read_to_string(path)else{return vec![]};serde_json::from_str::<Vec<String>>(&raw).unwrap_or_default().into_iter().map(PathBuf::from).filter(|p|p.is_dir()).collect()}
fn save_roots(app:&AppHandle,roots:&[PathBuf])->Result<(),String>{let path=config_file(app)?;let raw=serde_json::to_string_pretty(&roots.iter().map(|p|p.to_string_lossy().to_string()).collect::<Vec<_>>()).map_err(|e|e.to_string())?;fs::write(path,raw).map_err(|e|e.to_string())}
fn canonical(path:&Path)->Result<PathBuf,String>{fs::canonicalize(path).map_err(|e|format!("path unavailable: {e}"))}
fn under_root(state:&AppState,path:&Path)->Result<PathBuf,String>{let candidate=canonical(path)?;let roots=state.roots.lock().map_err(|_|"root lock failed".to_string())?;roots.iter().any(|root|candidate.starts_with(root)).then_some(candidate).ok_or_else(||"Path is outside authorized DownTrack locations".into())}
fn existing_or_parent(state:&AppState,path:&Path)->Result<PathBuf,String>{if path.exists(){return under_root(state,path)}let parent=path.parent().ok_or_else(||"Invalid path".to_string())?;let safe_parent=under_root(state,parent)?;let name=path.file_name().ok_or_else(||"Invalid path".to_string())?;Ok(safe_parent.join(name))}
fn root_views(roots:&[PathBuf])->Vec<RootView>{roots.iter().enumerate().map(|(i,p)|RootView{id:format!("root-{i}"),name:p.file_name().and_then(|x|x.to_str()).unwrap_or("Root").to_string(),path:p.to_string_lossy().to_string()}).collect()}

#[tauri::command]fn authorized_roots(state:State<AppState>)->Result<Vec<RootView>,String>{let g=state.roots.lock().map_err(|_|"root lock failed".to_string())?;Ok(root_views(&g))}
#[tauri::command]fn pick_root(app:AppHandle,state:State<AppState>)->Result<Option<RootView>,String>{let Some(folder)=rfd::FileDialog::new().set_title("Choose a DownTrack location").pick_folder()else{return Ok(None)};let root=canonical(&folder)?;if !root.is_dir(){return Err("Selected location is not a folder".into())}let mut g=state.roots.lock().map_err(|_|"root lock failed".to_string())?;if g.iter().any(|r|r==&root){return Ok(root_views(&g).into_iter().find(|r|r.path==root.to_string_lossy()))}g.push(root.clone());save_roots(&app,&g)?;Ok(root_views(&g).into_iter().find(|r|r.path==root.to_string_lossy()))}
#[tauri::command]fn remove_root(app:AppHandle,state:State<AppState>,path:String)->Result<(),String>{let root=canonical(Path::new(&path))?;let mut g=state.roots.lock().map_err(|_|"root lock failed".to_string())?;g.retain(|r|r!=&root);save_roots(&app,&g)}
#[tauri::command]fn list_directory(state:State<AppState>,path:String)->Result<Vec<FileEntry>,String>{let dir=under_root(&state,Path::new(&path))?;let mut out=vec![];for item in fs::read_dir(dir).map_err(|e|e.to_string())?{let entry=item.map_err(|e|e.to_string())?;let meta=entry.metadata().map_err(|e|e.to_string())?;let p=entry.path();let ext=p.extension().and_then(|x|x.to_str()).map(str::to_string);out.push(FileEntry{name:entry.file_name().to_string_lossy().to_string(),path:p.to_string_lossy().to_string(),is_dir:meta.is_dir(),size:if meta.is_file(){meta.len()}else{0},extension:ext});}out.sort_by_key(|x|(!x.is_dir,x.name.to_lowercase()));Ok(out)}
fn safe_child_name(name:&str)->Result<(),String>{let trimmed=name.trim();if trimmed.is_empty()||trimmed=="."||trimmed==".."||trimmed.contains('\\\\')||trimmed.contains('/')||trimmed.chars().any(|c|c.is_control()){return Err("Invalid name".into())}Ok(())}
#[tauri::command]fn create_directory(state:State<AppState>,parent:String,name:String)->Result<FileEntry,String>{safe_child_name(&name)?;let dir=under_root(&state,Path::new(&parent))?;let target=dir.join(&name);if target.exists(){return Err("An item with this name already exists".into())}fs::create_dir(&target).map_err(|e|e.to_string())?;Ok(FileEntry{name,path:target.to_string_lossy().to_string(),is_dir:true,size:0,extension:None})}
#[tauri::command]fn rename_entry(state:State<AppState>,path:String,new_name:String)->Result<(),String>{safe_child_name(&new_name)?;let src=under_root(&state,Path::new(&path))?;let parent=src.parent().ok_or_else(||"Invalid path".to_string())?;let dst=parent.join(new_name);if dst.exists(){return Err("An item with this name already exists".into())}fs::rename(src,dst).map_err(|e|e.to_string())}
#[tauri::command]fn delete_entry(state:State<AppState>,path:String)->Result<(),String>{let target=under_root(&state,Path::new(&path))?;let roots=state.roots.lock().map_err(|_|"root lock failed".to_string())?;if roots.iter().any(|r|r==&target){return Err("Authorized root folders cannot be deleted from DownTrack".into())}drop(roots);if target.is_dir(){fs::remove_dir_all(target).map_err(|e|e.to_string())}else{fs::remove_file(target).map_err(|e|e.to_string())}}
#[tauri::command]fn move_entry(state:State<AppState>,source:String,destination_parent:String)->Result<(),String>{let src=under_root(&state,Path::new(&source))?;let parent=under_root(&state,Path::new(&destination_parent))?;if src==parent||parent.starts_with(&src){return Err("Cannot move an item inside itself".into())}let name=src.file_name().ok_or_else(||"Invalid path".to_string())?;let dst=parent.join(name);if dst.exists(){return Err("An item with this name already exists".into())}fs::rename(src,dst).map_err(|e|e.to_string())}
fn copy_dir(src:&Path,dst:&Path)->Result<(),String>{fs::create_dir(dst).map_err(|e|e.to_string())?;for item in fs::read_dir(src).map_err(|e|e.to_string())?{let e=item.map_err(|e|e.to_string())?;let from=e.path();let to=dst.join(e.file_name());if from.is_dir(){copy_dir(&from,&to)?}else{fs::copy(&from,&to).map_err(|e|e.to_string())?}}Ok(())}
#[tauri::command]fn copy_entry(state:State<AppState>,source:String,destination_parent:String)->Result<(),String>{let src=under_root(&state,Path::new(&source))?;let parent=under_root(&state,Path::new(&destination_parent))?;if src==parent||parent.starts_with(&src){return Err("Cannot copy an item inside itself".into())}let name=src.file_name().ok_or_else(||"Invalid path".to_string())?;let dst=parent.join(name);if dst.exists(){return Err("An item with this name already exists".into())}if src.is_dir(){copy_dir(&src,&dst)}else{fs::copy(src,dst).map_err(|e|e.to_string()).map(|_|())}}
#[tauri::command]fn open_location(path:String)->Result<(),String>{#[cfg(target_os="windows")]{std::process::Command::new("explorer").arg(path).spawn().map_err(|e|e.to_string())?;}#[cfg(target_os="macos")]{std::process::Command::new("open").arg(path).spawn().map_err(|e|e.to_string())?;}#[cfg(target_os="linux")]{std::process::Command::new("xdg-open").arg(path).spawn().map_err(|e|e.to_string())?;}Ok(())}

fn parse_progress(line:&str)->Option<(f64,String,String)>{if !line.contains("[download]")||!line.contains('%'){return None}let mut percent=None;for token in line.split_whitespace(){let t=token.trim_end_matches('%');if let Ok(v)=t.parse::<f64>(){if v>=0.0&&v<=100.0{percent=Some(v);break}}}let speed=line.split(" at ").nth(1).and_then(|s|s.split_whitespace().next()).unwrap_or("").to_string();let eta=line.split(" ETA ").nth(1).and_then(|s|s.split_whitespace().next()).unwrap_or("").to_string();percent.map(|p|(p,speed,eta))}
fn emit_progress(app:&AppHandle,id:u64,status:&str,pct:f64,speed:&str,eta:&str,name:&str,msg:Option<String>){let _=app.emit("download-progress",DownloadEvent{id,status:status.into(),percent:pct,speed:speed.into(),eta:eta.into(),filename:name.into(),message:msg});}

async fn spawn_download(app:AppHandle,state:AppState,id:u64,request:DownloadRequest)->Result<(),String>{
 let d=under_root(&state,&request.destination)?;tokio::fs::create_dir_all(&d).await.map_err(|e|e.to_string())?;
 let out=d.join(&request.filename);let _=existing_or_parent(&state,&out)?;
 let selector=if request.format.eq_ignore_ascii_case("mp3"){"bestaudio/best".to_string()}else{format!("bestvideo[height<={}] + bestaudio/best",request.quality.trim_end_matches('p'))};
 let mut cmd=Command::new("yt-dlp");cmd.args(["--newline","--no-playlist","--continue","-f",&selector,"--merge-output-format",&request.format,"-o",out.to_string_lossy().as_ref(),&request.url]).stdout(Stdio::piped()).stderr(Stdio::null());
 let mut child=cmd.spawn().map_err(|e|format!("Media engine unavailable: {e}"))?;
 let stdout=child.stdout.take().ok_or_else(||"Unable to read media engine progress".to_string())?;
 let mut lines=BufReader::new(stdout).lines();let(tx,mut rx)=mpsc::channel::<Control>(4);
 {let mut jobs=state.downloads.jobs.lock().await;jobs.insert(id,DownloadJob{request:request.clone(),control:tx,status:"downloading".into()});}
 emit_progress(&app,id,"downloading",0.0,"","",&request.filename,None);
 let mut terminal_action:Option<&str>=None;
 loop{tokio::select!{
   line=lines.next_line()=>{match line{Ok(Some(line))=>{if let Some((p,s,e))=parse_progress(&line){emit_progress(&app,id,"downloading",p,&s,&e,&request.filename,None)}},Ok(None)=>break,Err(e)=>{let _=child.kill().await;terminal_action=Some("error");emit_progress(&app,id,"error",0.0,"","",&request.filename,Some(e.to_string()));break}}},
   control=rx.recv()=>{match control{Some(Control::Cancel)=>{let _=child.kill().await;terminal_action=Some("canceled");emit_progress(&app,id,"canceled",0.0,"","",&request.filename,None);break},Some(Control::Pause)=>{let _=child.kill().await;terminal_action=Some("paused");emit_progress(&app,id,"paused",0.0,"","",&request.filename,None);break},None=>break}}
 }}
 if terminal_action==Some("paused"){let _=child.wait().await;let mut jobs=state.downloads.jobs.lock().await;if let Some(job)=jobs.get_mut(&id){job.status="paused".into()}return Ok(())}
 let status=child.wait().await.map_err(|e|e.to_string())?;
 let final_status=if status.success(){"completed"}else if terminal_action==Some("canceled"){"canceled"}else{"error"};
 if final_status=="completed"{emit_progress(&app,id,final_status,100.0,"","",&request.filename,None)}else if final_status=="error"{emit_progress(&app,id,final_status,0.0,"","",&request.filename,Some("Download failed".into()))}
 state.downloads.jobs.lock().await.remove(&id);
 Ok(())
}

enum Control{Pause,Cancel}

#[tauri::command]
async fn start_download(app:AppHandle,state:State<'_,AppState>,mut req:DownloadRequest)->Result<u64,String>{
 let id=req.job_id.unwrap_or_else(||state.downloads.next_id.fetch_add(1,Ordering::Relaxed));req.job_id=Some(id);
 let tx=(mpsc::channel(4)).0;let mut jobs=state.downloads.jobs.lock().await;
 if jobs.contains_key(&id){return Err("Download already exists".into())}
 drop(jobs);
 let placeholder=(mpsc::channel(4)).0;
 let job=DownloadJob{request:req.clone(),control:placeholder,status:"starting".into()};
 state.downloads.jobs.lock().await.insert(id,job);
 let state_clone=state.inner().clone();let app_clone=app.clone();
 tokio::spawn(async move{let _=spawn_download(app_clone,state_clone,id,req).await;});
 Ok(id)
}

#[tauri::command]
async fn control_download(state:State<'_,AppState>,id:u64,action:String)->Result<(),String>{
 let sender={let jobs=state.downloads.jobs.lock().await;let job=jobs.get(&id).ok_or_else(||"Download not found".to_string())?;if job.status!="downloading"{return Err(format!("Download is {}",job.status))}job.control.clone()};let msg=match action.as_str(){"pause"=>Control::Pause,"cancel"=>Control::Cancel,_=>return Err("Unknown download action".into())};sender.send(msg).await.map_err(|_|"Download control channel closed".into())}
}

#[tauri::command]
async fn resume_download(app:AppHandle,state:State<'_,AppState>,req:DownloadRequest)->Result<(),String>{
 let id=req.job_id.ok_or_else(||"Download id required".to_string())?;
 {
   let jobs=state.downloads.jobs.lock().await;
   if jobs.contains_key(&id){return Err("Download is already active".into())}
 }
 let app_clone=app.clone();let state_clone=state.inner().clone();
 tokio::spawn(async move{let _=spawn_download(app_clone,state_clone,id,req).await;});
 Ok(())
}

#[tauri::command]
async fn inspect_media(url:String)->Result<MediaInfo,String>{let o=tokio::process::Command::new("yt-dlp").args(["--dump-single-json","--no-playlist",&url]).stdout(Stdio::piped()).stderr(Stdio::null()).output().await.map_err(|e|e.to_string())?;if !o.status.success(){return Err("Unable to inspect media".into())}let v:serde_json::Value=serde_json::from_slice(&o.stdout).map_err(|e|e.to_string())?;Ok(MediaInfo{title:v.get("title").and_then(|x|x.as_str()).unwrap_or("Untitled").into(),thumbnail:v.get("thumbnail").and_then(|x|x.as_str()).map(String::from),duration:v.get("duration_string").and_then(|x|x.as_str()).map(String::from),uploader:v.get("uploader").and_then(|x|x.as_str()).map(String::from)})}

fn main(){tauri::Builder::default().manage(AppState{roots:Arc::new(Mutex::new(Vec::new())),downloads:Arc::new(DownloadStore::default())}).setup(|app|{let roots=load_roots(app.handle());let state=app.state::<AppState>();*state.roots.lock().map_err(|_|"root lock failed")?=roots;Ok(())}).invoke_handler(tauri::generate_handler![authorized_roots,pick_root,remove_root,list_directory,create_directory,rename_entry,delete_entry,move_entry,copy_entry,open_location,start_download,control_download,resume_download,inspect_media]).run(tauri::generate_context!()).expect("DownTrack failed to start")}

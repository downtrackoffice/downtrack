use serde::{Deserialize,Serialize};
use std::{fs,io,path::{Path,PathBuf},process::Stdio,sync::{Arc,Mutex}};
use tauri::{AppHandle,Manager,State};

#[derive(Default,Clone)]
struct AppState{roots:Arc<Mutex<Vec<PathBuf>>>}

#[derive(Serialize,Clone)]
struct RootView{id:String,name:String,path:String}

#[derive(Serialize,Clone)]
struct FileEntry{name:String,path:String,is_dir:bool,size:u64,extension:Option<String>}

#[derive(Serialize)]
struct MediaInfo{title:String,thumbnail:Option<String>,duration:Option<String>}

#[derive(Deserialize)]
struct DownloadRequest{url:String,destination:PathBuf,filename:String,format:String,quality:String}

fn config_file(app:&AppHandle)->Result<PathBuf,String>{
    let dir=app.path().app_config_dir().map_err(|e|e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e|e.to_string())?;
    Ok(dir.join("roots.json"))
}

fn load_roots(app:&AppHandle)->Vec<PathBuf>{
    let Ok(path)=config_file(app) else{return Vec::new()};
    let Ok(raw)=fs::read_to_string(path) else{return Vec::new()};
    serde_json::from_str::<Vec<String>>(&raw).unwrap_or_default().into_iter().map(PathBuf::from).filter(|p|p.is_dir()).collect()
}

fn save_roots(app:&AppHandle,roots:&[PathBuf])->Result<(),String>{
    let path=config_file(app)?;
    let raw=serde_json::to_string_pretty(&roots.iter().map(|p|p.to_string_lossy().to_string()).collect::<Vec<_>>()).map_err(|e|e.to_string())?;
    fs::write(path,raw).map_err(|e|e.to_string())
}

fn canonical(path:&Path)->Result<PathBuf,String>{fs::canonicalize(path).map_err(|e|format!("path unavailable: {e}"))}

fn under_root(state:&AppState,path:&Path)->Result<PathBuf,String>{
    let candidate=canonical(path)?;
    let roots=state.roots.lock().map_err(|_|"root lock failed".to_string())?;
    roots.iter().any(|root|candidate.starts_with(root)).then_some(candidate).ok_or_else(||"Path is outside authorized DownTrack locations".into())
}

fn existing_or_parent(state:&AppState,path:&Path)->Result<PathBuf,String>{
    if path.exists(){return under_root(state,path)}
    let parent=path.parent().ok_or_else(||"Invalid path".to_string())?;
    let safe_parent=under_root(state,parent)?;
    let name=path.file_name().ok_or_else(||"Invalid path".to_string())?;
    Ok(safe_parent.join(name))
}

fn root_views(roots:&[PathBuf])->Vec<RootView>{
    roots.iter().enumerate().map(|(i,p)|RootView{id:format!("root-{i}"),name:p.file_name().and_then(|x|x.to_str()).unwrap_or_else(||p.to_str().unwrap_or("Root")).to_string(),path:p.to_string_lossy().to_string()}).collect()
}

#[tauri::command]
fn authorized_roots(app:AppHandle,state:State<AppState>)->Result<Vec<RootView>,String>{let g=state.roots.lock().map_err(|_|"root lock failed".to_string())?;Ok(root_views(&g))}

#[tauri::command]
fn pick_root(app:AppHandle,state:State<AppState>)->Result<Option<RootView>,String>{
    let Some(folder)=rfd::FileDialog::new().set_title("Choose a DownTrack location").pick_folder() else{return Ok(None)};
    let root=canonical(&folder)?;
    if !root.is_dir(){return Err("Selected location is not a folder".into())}
    let mut g=state.roots.lock().map_err(|_|"root lock failed".to_string())?;
    if g.iter().any(|r|r==&root){return Ok(root_views(&g).into_iter().find(|r|r.path==root.to_string_lossy()).ok())}
    g.push(root.clone());
    save_roots(&app,&g)?;
    Ok(root_views(&g).into_iter().find(|r|r.path==root.to_string_lossy()))
}

#[tauri::command]
fn remove_root(app:AppHandle,state:State<AppState>,path:String)->Result<(),String>{
    let root=canonical(Path::new(&path))?;
    let mut g=state.roots.lock().map_err(|_|"root lock failed".to_string())?;
    g.retain(|r|r!=&root);
    save_roots(&app,&g)
}

#[tauri::command]
fn list_directory(state:State<AppState>,path:String)->Result<Vec<FileEntry>,String>{
    let dir=under_root(&state,Path::new(&path))?;
    let mut out=Vec::new();
    for item in fs::read_dir(dir).map_err(|e|e.to_string())? {
        let entry=item.map_err(|e|e.to_string())?;
        let meta=entry.metadata().map_err(|e|e.to_string())?;
        let p=entry.path();
        let ext=p.extension().and_then(|x|x.to_str()).map(str::to_string);
        out.push(FileEntry{name:entry.file_name().to_string_lossy().to_string(),path:p.to_string_lossy().to_string(),is_dir:meta.is_dir(),size:if meta.is_file(){meta.len()}else{0},extension:ext});
    }
    out.sort_by_key(|x|(!x.is_dir,x.name.to_lowercase()));
    Ok(out)
}

fn safe_child_name(name:&str)->Result<(),String>{
    let trimmed=name.trim();
    if trimmed.is_empty()||trimmed=="."||trimmed==".."||trimmed.contains('\\\\') || trimmed.contains('/'){return Err("Invalid name".into())}
    Ok(())
}

#[tauri::command]
fn create_directory(state:State<AppState>,parent:String,name:String)->Result<FileEntry,String>{
    safe_child_name(&name)?;
    let dir=under_root(&state,Path::new(&parent))?;
    let target=dir.join(&name);
    if target.exists(){return Err("An item with this name already exists".into())}
    fs::create_dir(&target).map_err(|e|e.to_string())?;
    let m=target.metadata().map_err(|e|e.to_string())?;
    Ok(FileEntry{name,path:target.to_string_lossy().to_string(),is_dir:true,size:0,extension:None})
}

#[tauri::command]
fn rename_entry(state:State<AppState>,path:String,new_name:String)->Result<(),String>{
    safe_child_name(&new_name)?;
    let src=under_root(&state,Path::new(&path))?;
    let parent=src.parent().ok_or_else(||"Invalid path".to_string())?;
    let dst=parent.join(&new_name);
    let dst=existing_or_parent(&state,&dst)?;
    if dst.exists(){return Err("An item with this name already exists".into())}
    fs::rename(src,dst).map_err(|e|e.to_string())
}

#[tauri::command]
fn delete_entry(state:State<AppState>,path:String)->Result<(),String>{
    let target=under_root(&state,Path::new(&path))?;
    let roots=state.roots.lock().map_err(|_|"root lock failed".to_string())?;
    if roots.iter().any(|r|r==&target){return Err("Authorized root folders cannot be deleted from DownTrack".into())}
    let meta=target.metadata().map_err(|e|e.to_string())?;
    if meta.is_dir(){fs::remove_dir_all(target).map_err(|e|e.to_string())}else{fs::remove_file(target).map_err(|e|e.to_string())}
}

#[tauri::command]
fn move_entry(state:State<AppState>,source:String,destination_parent:String)->Result<(),String>{
    let src=under_root(&state,Path::new(&source))?;
    let parent=under_root(&state,Path::new(&destination_parent))?;
    let name=src.file_name().ok_or_else(||"Invalid path".to_string())?;
    let dst=parent.join(name);
    if dst.exists(){return Err("An item with this name already exists".into())}
    fs::rename(src,dst).map_err(|e|e.to_string())
}

#[tauri::command]
fn copy_entry(state:State<AppState>,source:String,destination_parent:String)->Result<(),String>{
    let src=under_root(&state,Path::new(&source))?;
    let parent=under_root(&state,Path::new(&destination_parent))?;
    let name=src.file_name().ok_or_else(||"Invalid path".to_string())?;
    let dst=parent.join(name);
    if dst.exists(){return Err("An item with this name already exists".into())}
    if src.is_dir(){copy_dir(&src,&dst)?}else{fs::copy(src,dst).map_err(|e|e.to_string())?;};
    Ok(())
}

fn copy_dir(src:&Path,dst:&Path)->Result<(),String>{
    fs::create_dir(dst).map_err(|e|e.to_string())?;
    for item in fs::read_dir(src).map_err(|e|e.to_string())? {
        let e=item.map_err(|e|e.to_string())?;
        let from=e.path(); let to=dst.join(e.file_name());
        if from.is_dir(){copy_dir(&from,&to)?}else{fs::copy(&from,&to).map_err(|e|e.to_string())?;}
    }
    Ok(())
}

#[tauri::command]
async fn inspect_media(url:String)->Result<MediaInfo,String>{
    let o=tokio::process::Command::new("yt-dlp").args(["--dump-single-json","--no-playlist",&url]).stdout(Stdio::piped()).stderr(Stdio::null()).output().await.map_err(|e|e.to_string())?;
    if !o.status.success(){return Err("Unable to inspect media".into())}
    let v:serde_json::Value=serde_json::from_slice(&o.stdout).map_err(|e|e.to_string())?;
    Ok(MediaInfo{title:v.get("title").and_then(|x|x.as_str()).unwrap_or("Untitled").into(),thumbnail:v.get("thumbnail").and_then(|x|x.as_str()).map(String::from),duration:v.get("duration_string").and_then(|x|x.as_str()).map(String::from)})
}

#[tauri::command]
async fn start_download(state:State<'_,AppState>,req:DownloadRequest)->Result<(),String>{
    let d=under_root(&state,&req.destination)?;
    tokio::fs::create_dir_all(&d).await.map_err(|e|e.to_string())?;
    let out=d.join(&req.filename);
    let _=existing_or_parent(&state,&out)?;
    let selector=if req.format.eq_ignore_ascii_case("mp3"){"bestaudio/best".into()}else{format!("bestvideo[height<={}] + bestaudio/best",req.quality.trim_end_matches('p'))};
    let s=tokio::process::Command::new("yt-dlp").args(["--newline","--no-playlist","-f",&selector,"--merge-output-format",&req.format,"-o",out.to_string_lossy().as_ref(),&req.url]).stdout(Stdio::null()).stderr(Stdio::null()).status().await.map_err(|e|e.to_string())?;
    if s.success(){Ok(())}else{Err("Download failed".into())}
}

fn main(){
    tauri::Builder::default()
      .manage(AppState{roots:Arc::new(Mutex::new(Vec::new()))})
      .setup(|app|{
        let roots=load_roots(app.handle());
        let state=app.state::<AppState>();
        *state.roots.lock().map_err(|_|"root lock failed")?=roots;
        Ok(())
      })
      .invoke_handler(tauri::generate_handler![authorized_roots,pick_root,remove_root,list_directory,create_directory,rename_entry,delete_entry,move_entry,copy_entry,inspect_media,start_download])
      .run(tauri::generate_context!())
      .expect("DownTrack failed to start")
}

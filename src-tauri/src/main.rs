use serde::{Deserialize,Serialize};
use std::{path::{Path,PathBuf},process::Stdio,sync::{Arc,Mutex}};
use tauri::State;

#[derive(Default,Clone)] struct AppState{roots:Arc<Mutex<Vec<PathBuf>>>}
#[derive(Serialize)] struct MediaInfo{title:String,thumbnail:Option<String>,duration:Option<String>,formats:Vec<FormatInfo>}
#[derive(Serialize)] struct FormatInfo{id:String,label:String,extension:String,size:Option<String>}
#[derive(Deserialize)] struct DownloadRequest{url:String,destination:PathBuf,filename:String,format:String,quality:String}

fn canonical(p:&Path)->Result<PathBuf,String>{std::fs::canonicalize(p).map_err(|e|format!("path unavailable: {e}"))}
fn allowed(state:&AppState,target:&Path)->Result<PathBuf,String>{let target=canonical(target)?;let roots=state.roots.lock().map_err(|_|"root lock failed".to_string())?;if roots.iter().any(|r|target.starts_with(r)){Ok(target)}else{Err("Destination is outside authorized DownTrack locations".into())}}

#[tauri::command]
fn set_authorized_roots(state:State<AppState>,roots:Vec<String>)->Result<Vec<String>,String>{let mut guard=state.roots.lock().map_err(|_|"root lock failed".to_string())?;let mut accepted=Vec::new();for raw in roots{let p=canonical(Path::new(&raw))?;if p.is_dir(){accepted.push(p.to_string_lossy().to_string())}}*guard=accepted.iter().map(PathBuf::from).collect();Ok(accepted)}

#[tauri::command]
fn authorized_roots(state:State<AppState>)->Result<Vec<String>,String>{let guard=state.roots.lock().map_err(|_|"root lock failed".to_string())?;Ok(guard.iter().map(|p|p.to_string_lossy().to_string()).collect())}

#[tauri::command]
fn list_directory(state:State<AppState>,path:String)->Result<Vec<String>,String>{let dir=allowed(&state,Path::new(&path))?;let mut out=Vec::new();for e in std::fs::read_dir(dir).map_err(|e|e.to_string())?{let e=e.map_err(|e|e.to_string())?;out.push(e.path().to_string_lossy().to_string())}Ok(out)}

#[tauri::command]
async fn inspect_media(url:String)->Result<MediaInfo,String>{let output=tokio::process::Command::new("yt-dlp").args(["--dump-single-json","--no-playlist",&url]).stdout(Stdio::piped()).stderr(Stdio::null()).output().await.map_err(|e|format!("media engine unavailable: {e}"))?;if !output.status.success(){return Err("Unable to inspect this media URL".into())}let v:serde_json::Value=serde_json::from_slice(&output.stdout).map_err(|e|e.to_string())?;let formats=v.get("formats").and_then(|x|x.as_array()).cloned().unwrap_or_default().into_iter().filter_map(|f|{let id=f.get("format_id")?.as_str()?.to_string();let ext=f.get("ext").and_then(|x|x.as_str()).unwrap_or("media").to_string();let height=f.get("height").and_then(|x|x.as_u64()).map(|h|format!("{}p",h));Some(FormatInfo{id:id.clone(),label:height.unwrap_or_else(||"Audio".into()),extension:ext,size:f.get("filesize").and_then(|x|x.as_u64()).map(|n|n.to_string())})}).collect();Ok(MediaInfo{title:v.get("title").and_then(|x|x.as_str()).unwrap_or("Untitled media").into(),thumbnail:v.get("thumbnail").and_then(|x|x.as_str()).map(String::from),duration:v.get("duration_string").and_then(|x|x.as_str()).map(String::from),formats})}

#[tauri::command]
async fn start_download(state:State<'_,AppState>,req:DownloadRequest)->Result<(),String>{let dir=allowed(&state,&req.destination)?;tokio::fs::create_dir_all(&dir).await.map_err(|e|e.to_string())?;let output=dir.join(&req.filename);let template=output.to_string_lossy().to_string();let selector=if req.format.eq_ignore_ascii_case("mp3"){"bestaudio/best".to_string()}else{format!("bestvideo[height<={}] + bestaudio/best",req.quality.trim_end_matches('p'))};let status=tokio::process::Command::new("yt-dlp").args(["--newline","--no-playlist","-f",&selector,"--merge-output-format",&req.format,"-o",&template,&req.url]).stdout(Stdio::null()).stderr(Stdio::null()).status().await.map_err(|e|format!("download engine unavailable: {e}"))?;if status.success(){Ok(())}else{Err("Download failed".into())}}

fn main(){tauri::Builder::default().manage(AppState::default()).invoke_handler(tauri::generate_handler![set_authorized_roots,authorized_roots,list_directory,inspect_media,start_download]).run(tauri::generate_context!()).expect("error while running DownTrack");}

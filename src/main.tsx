import React,{useEffect,useMemo,useState}from'react';
import{createRoot}from'react-dom/client';
import{invoke}from'@tauri-apps/api/core';import{listen,UnlistenFn}from'@tauri-apps/api/event';
import{Folder,FolderPlus,Download,Settings,Search,Grid2X2,List,Plus,ChevronRight,Home,Music,Film,ShieldCheck,Pause,Play,X,Check,Globe2,LockKeyhole,MoreVertical,RefreshCw,Sun,Moon,File,ArrowUp,Trash2,Copy,Scissors,PenLine}from'lucide-react';
import'./styles.css';

type Root={id:string;name:string;path:string};
type Entry={name:string;path:string;is_dir:boolean;size:number;extension?:string|null};
type Job={id:number;name:string;type:string;progress:number;speed:string;eta:string;status:'downloading'|'done'|'paused'|'error'|'canceled';url:string;destination:string;filename:string;format:string;quality:string;createdAt:number};

const localeFallback={app:'DownTrack',add:'הוספת מדיה',locations:'מיקומים',downloads:'הורדות',settings:'הגדרות',search:'חיפוש בקבצים...',protected:'Workspace מוגן'};
const formatBytes=(n:number)=>n>=1e9?(n/1e9).toFixed(2)+' GB':n>=1e6?(n/1e6).toFixed(1)+' MB':n>=1e3?(n/1e3).toFixed(1)+' KB':n+' B';
const iconFor=(e:Entry)=><>{e.is_dir?<Folder/>:e.extension?.toLowerCase()==='mp3'?<Music/>:['mp4','mkv','webm','mov','avi'].includes(e.extension?.toLowerCase()||'')?<Film/>:<File/>}</>;

function App(){
 const[theme,setTheme]=useState<'dark'|'light'>(()=>{try{return(localStorage.getItem('downtrack-theme')as 'dark'|'light')||'dark'}catch{return'dark'}});
 const[roots,setRoots]=useState<Root[]>([]),[currentPath,setCurrentPath]=useState(''),[entries,setEntries]=useState<Entry[]>([]);
 const[view,setView]=useState<'grid'|'list'>('grid'),[query,setQuery]=useState(''),[loading,setLoading]=useState(true);
 const[add,setAdd]=useState(false),[settings,setSettings]=useState(false),[error,setError]=useState('');
 const[jobs,setJobs]=useState<Job[]>(()=>{try{const saved=JSON.parse(localStorage.getItem('downtrack-jobs')||'[]') as Job[];return saved.map(j=>j.status==='downloading'?{...j,status:'paused'}:j)}catch{return[]}}),[selected,setSelected]=useState<Entry|null>(null);

 const applyTheme=(next:'dark'|'light')=>{setTheme(next);try{localStorage.setItem('downtrack-theme',next)}catch{}};
 const currentRoot=useMemo(()=>roots.find(r=>currentPath===r.path||currentPath.startsWith(r.path+'\\')||currentPath.startsWith(r.path+'/')), [roots,currentPath]);
 const breadcrumbs=useMemo(()=>{if(!currentRoot)return[];if(currentPath===currentRoot.path)return[{name:currentRoot.name,path:currentRoot.path}];const rel=currentPath.slice(currentRoot.path.length).replace(/^[/\\]+/,'');const parts=rel.split(/[/\\]+/).filter(Boolean);let p=currentRoot.path;return[{name:currentRoot.name,path:p},...parts.map(name=>{p=p.replace(/[\\/]$/,'')+'\\'+name;return{name,path:p}})]},[currentRoot,currentPath]);
 const visibleEntries=useMemo(()=>entries.filter(e=>e.name.toLowerCase().includes(query.toLowerCase())),[entries,query]);

 const refreshRoots=async()=>{try{const r=await invoke<Root[]>('authorized_roots');setRoots(r);if(!currentPath&&r[0])setCurrentPath(r[0].path);if(currentPath&&!r.some(x=>currentPath===x.path||currentPath.startsWith(x.path+'\\')||currentPath.startsWith(x.path+'/')))setCurrentPath(r[0]?.path||'')}catch(e){setError(String(e))}};
 const loadDir=async(path:string)=>{if(!path)return;setLoading(true);setError('');try{setEntries(await invoke<Entry[]>('list_directory',{path}));setCurrentPath(path)}catch(e){setError(String(e));setEntries([])}finally{setLoading(false)}};

 useEffect(()=>{try{localStorage.setItem('downtrack-jobs',JSON.stringify(jobs))}catch{}},[jobs]);
 useEffect(()=>{refreshRoots().finally(()=>setLoading(false));let off:UnlistenFn|undefined;listen<any>('download-progress',event=>{const e=event.payload as {id:number;status:string;percent:number;speed:string;eta:string;filename:string};setJobs(prev=>{const existing=prev.find(j=>j.id===e.id);const status=e.status==='completed'?'done':e.status==='canceled'?'canceled':e.status==='paused'?'paused':e.status==='error'?'error':'downloading';if(existing)return prev.map(j=>j.id===e.id?{...j,progress:e.percent,speed:e.speed,eta:e.eta,status}:j);return[{id:e.id,name:e.filename,type:'Media',progress:e.percent,speed:e.speed,eta:e.eta,status},...prev]})}).then(x=>off=x).catch(()=>{});return()=>{if(off)off()};},[]);
 useEffect(()=>{if(currentPath)loadDir(currentPath)},[currentPath]);

 const addRoot=async()=>{try{const root=await invoke<Root|null>('pick_root');if(root){await refreshRoots();setCurrentPath(root.path)}}catch(e){setError(String(e))}};
 const removeRoot=async(root:Root)=>{if(!confirm('להסיר את המיקום המורשה מ־DownTrack? הקבצים עצמם לא יימחקו.'))return;try{await invoke('remove_root',{path:root.path});await refreshRoots()}catch(e){setError(String(e))}};
 const newFolder=async()=>{const name=prompt('שם התיקייה החדשה');if(!name)return;try{await invoke('create_directory',{parent:currentPath,name});await loadDir(currentPath)}catch(e){setError(String(e))}};
 const renameSelected=async()=>{if(!selected)return;const name=prompt('שם חדש',selected.name);if(!name||name===selected.name)return;try{await invoke('rename_entry',{path:selected.path,newName:name});setSelected(null);await loadDir(currentPath)}catch(e){setError(String(e))}};
 const copySelected=async()=>{if(!selected)return;const dest=prompt('תיקיית יעד',currentPath);if(!dest)return;try{await invoke('copy_entry',{source:selected.path,destinationParent:dest});await loadDir(currentPath)}catch(e){setError(String(e))}};
 const moveSelected=async()=>{if(!selected)return;const dest=prompt('תיקיית יעד',currentPath);if(!dest)return;try{await invoke('move_entry',{source:selected.path,destinationParent:dest});setSelected(null);await loadDir(currentPath)}catch(e){setError(String(e))}};
 const controlJob=async(j:Job)=>{try{if(j.status==='paused'){await invoke('resume_download',{req:{url:j.url,destination:j.destination,filename:j.filename,format:j.format,quality:j.quality,job_id:j.id,resume:true}});setJobs(prev=>prev.map(x=>x.id===j.id?{...x,status:'downloading',eta:'מתחדש…'}:x))}else if(j.status==='downloading'){await invoke('control_download',{id:j.id,action:'pause'});setJobs(prev=>prev.map(x=>x.id===j.id?{...x,status:'paused'}:x))}else{await invoke('control_download',{id:j.id,action:'cancel'});setJobs(prev=>prev.map(x=>x.id===j.id?{...x,status:'canceled'}:x))}}catch(e){setError(String(e))}};
 const deleteSelected=async()=>{if(!selected)return;const what=selected.is_dir?'התיקייה וכל התוכן שלה':'הקובץ';if(!confirm('למחוק את '+what+'?'))return;try{await invoke('delete_entry',{path:selected.path});setSelected(null);await loadDir(currentPath)}catch(e){setError(String(e))}};
 const toggleTheme=()=>applyTheme(theme==='dark'?'light':'dark');

 return <div className={'app '+theme}>
 <aside className="sidebar"><div className="brand"><div className="logo">▶</div><div><b>DownTrack</b><span>Media Workspace</span></div></div>
 <div className="section-label">{localeFallback.locations}</div>
 <button className="nav active" onClick={()=>roots[0]&&loadDir(roots[0].path)}><Home/>{localeFallback.app}</button>
 {roots.map(r=><button key={r.id} className={'nav '+(currentRoot?.id===r.id?'selected':'')} onClick={()=>loadDir(r.path)}><Folder/>{r.name}</button>)}
 <button className="nav"><Download/> {localeFallback.downloads}<em>{jobs.filter(j=>j.status==='downloading').length}</em></button>
 <div className="side-spacer"/>
 <button className="nav accent" onClick={()=>setAdd(true)}><Plus/>{localeFallback.add}</button>
 <button className="nav" onClick={()=>setSettings(true)}><Settings/>{localeFallback.settings}</button>
 <div className="secure"><ShieldCheck/><div><b>{localeFallback.protected}</b><span>גישה רק לתיקיות מאושרות</span></div></div></aside>

 <main className="main"><header><div className="crumbs">{breadcrumbs.map((b,i)=><React.Fragment key={b.path}>{i>0&&<ChevronRight size={15}/>}<button onClick={()=>loadDir(b.path)}>{i===0?<Home size={16}/>:b.name}</button></React.Fragment>)}</div>
 <div className="search"><Search size={17}/><input value={query} onChange={e=>setQuery(e.target.value)} placeholder={localeFallback.search}/></div>
 <div className="toolbar"><button onClick={()=>setView('grid')} className={view==='grid'?'on':''}><Grid2X2/></button><button onClick={()=>setView('list')} className={view==='list'?'on':''}><List/></button><button onClick={()=>currentPath&&loadDir(currentPath)}><RefreshCw/></button><button className="theme-toggle" onClick={toggleTheme}>{theme==='dark'?<Sun/>:<Moon/>}</button></div></header>

 {error&&<div className="error-banner"><span>{error}</span><button onClick={()=>setError('')}><X size={15}/></button></div>}
 {!currentPath&&roots.length===0&&!loading?<EmptyState onAdd={addRoot}/>:<section className="content">
 <div className="title-row"><div><h1>{currentRoot?.name||'Workspace'}</h1><p>{currentPath} · כל תיקיות המשנה מורשות</p></div><div className="action-row"><button className="secondary top-action" onClick={newFolder}><FolderPlus/>תיקייה חדשה</button><button className="primary" onClick={()=>setAdd(true)}><Plus/>הוספת מדיה</button></div></div>
 <div className="file-tools"><button className="up-button" onClick={()=>breadcrumbs.length>1&&loadDir(breadcrumbs[breadcrumbs.length-2].path)} disabled={breadcrumbs.length<=1}><ArrowUp/> למעלה</button>{selected?<><span className="selected-label">{selected.name}</span><button onClick={renameSelected}><PenLine/> שינוי שם</button><button onClick={deleteSelected}><Trash2/> מחיקה</button><button onClick={copySelected}><Copy/> העתק</button><button onClick={moveSelected}><Scissors/> העבר</button></>:<span>{visibleEntries.length} פריטים</span>}</div>
 {loading?<div className="loading">טוען תיקייה…</div>:view==='grid'?<div className="file-grid">{visibleEntries.map(e=><article key={e.path} onClick={()=>setSelected(e)} onDoubleClick={()=>e.is_dir&&loadDir(e.path)} className={'file-card '+(selected?.path===e.path?'selected-card':'')}><div className={e.is_dir?'folder-icon':'thumb'}>{iconFor(e)}</div><b title={e.name}>{e.name}</b><span>{e.is_dir?'תיקייה':(e.extension?.toUpperCase()||'קובץ')+' · '+formatBytes(e.size)}</span><button className="more" onClick={ev=>{ev.stopPropagation();setSelected(e)}}><MoreVertical/></button></article>)}</div>:<div className="file-list">{visibleEntries.map(e=><button className={'list-row '+(selected?.path===e.path?'selected-row':'')} key={e.path} onClick={()=>setSelected(e)} onDoubleClick={()=>e.is_dir&&loadDir(e.path)}><span className="list-icon">{iconFor(e)}</span><span className="list-name">{e.name}</span><span>{e.is_dir?'תיקייה':e.extension?.toUpperCase()||'קובץ'}</span><span>{e.is_dir?'—':formatBytes(e.size)}</span></button>)}</div>}
 </section>}
 <div className="downloads-panel"><div className="panel-head"><div><b>הורדות פעילות</b><span>{jobs.filter(j=>j.status==='downloading').length} פעילות</span></div><button>הצג הכל <ChevronRight/></button></div><div className="job-row">{jobs.length===0?<div className="empty-jobs">אין כרגע הורדות</div>:jobs.slice(0,4).map(j=><div className="job" key={j.id}><div className="job-icon"><Film/></div><div className="job-info"><b>{j.name}</b><span>{j.type}</span><div className="progress"><i style={{width:j.progress+'%'}}/></div><small>{j.progress}% · {j.speed} · {j.eta}</small></div><button onClick={()=>controlJob(j)}>{j.status==='paused'?<Play/>:<Pause/>}</button><button onClick={()=>controlJob({...j,status:'canceled'})}><X/></button></div>)}</div></div>
 </main>
 {add&&<AddMedia onClose={()=>setAdd(false)} target={currentPath} onQueued={j=>setJobs(x=>[j,...x])}/>}
 {settings&&<SettingsModal onClose={()=>setSettings(false)} theme={theme} setTheme={applyTheme} roots={roots} addRoot={addRoot} removeRoot={removeRoot}/>}
 </div>
}

function EmptyState({onAdd}:{onAdd:()=>void}){return <div className="empty-state"><div className="empty-logo"><ShieldCheck/></div><h2>ברוכים הבאים ל־DownTrack</h2><p>כדי להתחיל, הוסף תיקייה ראשית אחת או יותר. DownTrack יקבל גישה רק למיקומים האלה ולכל מה שנמצא בתוכם.</p><button className="primary" onClick={onAdd}><FolderPlus/>הוסף מיקום</button><small>אפשר להוסיף או להסיר מיקומים בכל עת</small></div>}

function AddMedia({onClose,target,onQueued}:{onClose:()=>void;target:string;onQueued:(j:Job)=>void}){const[videoUrl,setVideoUrl]=useState(''),[quality,setQuality]=useState('1080p'),[format,setFormat]=useState('MP4'),[name,setName]=useState(''),[info,setInfo]=useState<{title:string;thumbnail?:string|null;duration?:string|null}|null>(null),[busy,setBusy]=useState(false),[err,setErr]=useState('');
 const inspect=async()=>{if(!videoUrl.trim())return;setBusy(true);setErr('');try{const v=await invoke<any>('inspect_media',{url:videoUrl.trim()});setInfo(v);if(!name)setName(v.title)}catch(e){setErr('לא ניתן לזהות את הקישור.')}finally{setBusy(false)}};
 const queue=async()=>{if(!videoUrl.trim()||!target||!name.trim())return;const filename=name.trim()+'.'+format.toLowerCase();try{const id=await invoke<number>('start_download',{req:{url:videoUrl.trim(),destination:target,filename,format,quality}});onQueued({id,name:filename,type:quality+' • '+format,progress:0,speed:'',eta:'מכין…',status:'downloading',url:videoUrl.trim(),destination:target,filename,format,quality,createdAt:Date.now()});onClose()}catch(e){setErr('לא ניתן להתחיל את ההורדה.')}};
 return <div className="overlay"><div className="modal add-modal"><div className="modal-head"><div><h2>הוספת מדיה</h2><span>ההורדה תישמר ישירות בתיקייה הנוכחית</span></div><button onClick={onClose}><X/></button></div>
 <label>קישור<input autoFocus value={videoUrl} onChange={e=>setVideoUrl(e.target.value)} onKeyDown={e=>e.key==='Enter'&&inspect()} placeholder="הדבק קישור למדיה..."/></label>
 <button className="inspect-button" onClick={inspect} disabled={busy}>{busy?'בודק…':'זיהוי מדיה'}</button>
 {info&&<div className="preview">{info.thumbnail?<img src={info.thumbnail} alt=""/>:<div className="preview-img"><Film/></div>}<div><b>{info.title}</b><span>{info.duration||'מדיה'} · זוהה בהצלחה</span></div><Check/></div>}
 {err&&<div className="modal-error">{err}</div>}
 <label>איכות<select value={quality} onChange={e=>setQuality(e.target.value)}><option>2160p</option><option>1440p</option><option>1080p</option><option>720p</option><option>480p</option></select></label>
 <div className="formats"><label>פורמט</label><div>{['MP4','MKV','MP3'].map(x=><button key={x} className={format===x?'picked':''} onClick={()=>setFormat(x)}>{x}</button>)}</div></div>
 <label>שם הקובץ<input value={name} onChange={e=>setName(e.target.value)} placeholder="שם הקובץ"/></label>
 <div className="destination"><Folder/><div><span>יישמר בתוך</span><b>{target||'לא נבחר מיקום'}</b></div><Check/></div>
 <button className="primary wide" disabled={!target||!name.trim()||!videoUrl.trim()} onClick={queue}><Download/>הוסף להורדות</button></div></div>
}

function SettingsModal({onClose,theme,setTheme,roots,addRoot,removeRoot}:{onClose:()=>void;theme:'dark'|'light';setTheme:(t:'dark'|'light')=>void;roots:Root[];addRoot:()=>void;removeRoot:(r:Root)=>void}){return <div className="overlay"><div className="modal settings-modal"><div className="modal-head"><div><h2>הגדרות</h2><span>שליטה מלאה ב־DownTrack</span></div><button onClick={onClose}><X/></button></div><div className="settings-grid"><div className="setting active"><Globe2/><div><b>שפה</b><span>עברית · תשתית ל־20 שפות</span></div></div><div className="setting"><Sun/><div><b>עיצוב</b><span>בהיר או כהה</span><div className="theme-options"><button className={theme==='dark'?'picked':''} onClick={()=>setTheme('dark')}><Moon/>כהה</button><button className={theme==='light'?'picked':''} onClick={()=>setTheme('light')}><Sun/>בהיר</button></div></div></div><div className="setting"><LockKeyhole/><div><b>פרטיות ואבטחה</b><span>גישה דרך Roots בלבד</span></div></div><div className="setting"><Download/><div><b>הורדות</b><span>תורים, חיבורים ופורמטים</span></div></div></div><div className="roots"><h3>מיקומים מורשים</h3>{roots.map(r=><div className="root-row" key={r.id}><Folder/><div><b>{r.name}</b><span>{r.path} · כולל כל תיקיות המשנה</span></div><ShieldCheck/><button className="root-remove" onClick={()=>removeRoot(r)}><X/></button></div>)}<button className="secondary" onClick={addRoot}><FolderPlus/>הוסף מיקום</button></div></div></div>}

createRoot(document.getElementById('root')!).render(<App/>);

//! Docker is queried only by explicit palette requests. No watcher or idle worker.
use std::{path::{Path,PathBuf}, process::{Command,Stdio}, sync::{mpsc,Arc,atomic::{AtomicBool,Ordering}}, time::{Duration,Instant}, io::Read};
use eframe::egui;

pub const RETURN_TO_LOCAL_SCRIPT:&str=r#"trap ' : ' INT; "$@"; code=$?; printf '\033]2;ATELIER_DOCKER:%s:local\007' "$ATELIER_SSH_TOKEN"; if (( code )); then printf '\nDocker a terminé avec le code %s\n' "$code"; fi; exec /bin/zsh -l"#;

/// Authenticated OSC from the private Docker relay, never from prompt guessing.
pub fn parse_relay(title: &str, token: &str) -> Option<Badge> {
    let value = title.strip_prefix("ATELIER_DOCKER:")?.strip_prefix(token)?.strip_prefix(":exec:")?;
    let (endpoint, container) = value.split_once('|')?;
    if container.is_empty() || value.chars().any(char::is_control) { return None; }
    Some(Badge { machine: Machine::from_endpoint(endpoint), label: format!("DOCKER · {}", container.chars().take(200).collect::<String>()) })
}

pub fn compact_badge(text:&str)->String {
    let clean:String=text.chars().filter(|c|!c.is_control()).collect();
    if clean.chars().count()>32 {format!("{}…",clean.chars().take(31).collect::<String>())}else{clean}
}

#[derive(Clone,Debug,PartialEq)]
pub enum Machine { Local, Ssh(String), Remote(String), Unknown }
impl Machine {
    pub fn from_endpoint(endpoint:&str)->Self {
        if endpoint.starts_with("unix://") { Self::Local }
        else if let Some(host)=endpoint.strip_prefix("ssh://").filter(|s|!s.is_empty()) {Self::Ssh(host.to_owned())}
        else if endpoint.starts_with("tcp://") || endpoint.starts_with("https://") || endpoint.starts_with("http://") {Self::Remote(endpoint.to_owned())}
        else {Self::Unknown}
    }
    pub fn label(&self)->String {match self {Self::Local=>"LOCAL".into(),Self::Ssh(h)=>format!("SSH · {h}"),Self::Remote(h)=>format!("MOTEUR · {h}"),Self::Unknown=>"HÔTE NON IDENTIFIÉ".into()}}
}
#[derive(Clone,Debug)]
pub struct Binding {pub directory:PathBuf,pub docker:PathBuf,pub target:Vec<String>,pub machine:Machine,pub context:String,pub files:Vec<PathBuf>}
impl Binding {
    pub fn compose_args(&self,tail:&[&str])->Vec<String> {
        let mut args=self.target.clone();args.push("compose".into());
        for file in &self.files {args.extend(["-f".into(),file.to_string_lossy().into_owned()]);}
        args.extend(tail.iter().map(|s|s.to_string()));args
    }
    pub fn describe(&self)->String {format!("{} · {} · {}",self.directory.file_name().unwrap_or_default().to_string_lossy(),self.context,self.machine.label())}
}
#[derive(Clone,Debug,serde::Deserialize)]
pub struct Container {
    #[serde(rename="ID",default)] pub id:String,
    #[serde(rename="Name",default)] pub name:String,
    #[serde(rename="Service",default)] pub service:String,
    #[serde(rename="State",default)] pub state:String,
    #[serde(rename="Health",default)] pub health:String,
}
#[derive(Clone,Debug)]
pub struct Snapshot {pub binding:Binding,pub services:Vec<String>,pub containers:Vec<Container>,pub taken:Instant}
#[derive(Clone,Debug)]
pub enum Action { Open, Refresh, Back, Up, Stop, Restart(String), Shell(Container), Logs(String), Configure }
#[derive(Clone)]
pub struct Badge {pub machine:Machine,pub label:String}
#[derive(Default)]
pub struct Browser {
    pub active:bool,pub root:Option<PathBuf>,pub snapshot:Option<Snapshot>,pub message:Option<String>,
    pub file_selections:std::collections::BTreeMap<PathBuf,Vec<PathBuf>>,
    pub loading:bool,pub files:Vec<PathBuf>,pub configure:bool,pub file_input:String,
    receiver:Option<mpsc::Receiver<Result<Snapshot,String>>>,cancel:Option<Arc<AtomicBool>>,
}
impl Browser {
    pub fn leave(&mut self) {self.active=false;self.configure=false;self.cancel();}
    fn cancel(&mut self) {if let Some(c)=self.cancel.take(){c.store(true,Ordering::Relaxed);}self.receiver=None;self.loading=false;}
    pub fn open(&mut self,root:PathBuf,ctx:egui::Context) {
        self.cancel(); if self.root.as_ref()!=Some(&root){self.files=self.file_selections.get(&root).cloned().unwrap_or_default();self.file_input=self.files.iter().map(|p|p.to_string_lossy()).collect::<Vec<_>>().join("\n");}
        self.root=Some(root);self.snapshot=None;self.active=true;self.refresh(ctx);
    }
    pub fn refresh(&mut self,ctx:egui::Context) {
        let Some(root)=self.root.clone() else{return};let files=self.files.clone();
        self.start(ctx,move |cancel|{let binding=resolve(&root,files,cancel)?;query(binding,cancel) });
    }
    pub fn mutate(&mut self,action:Action,ctx:egui::Context) {
        let Some(snapshot)=self.snapshot.as_ref() else{return};let binding=snapshot.binding.clone();
        self.start(ctx,move |cancel|{
            let args=match action {Action::Up=>binding.compose_args(&["up","-d"]),Action::Stop=>binding.compose_args(&["stop"]),Action::Restart(service)=>binding.compose_args(&["restart","--",&service]),_=>return Err("Action invalide".into())};
            run(&binding.docker,&args,&binding.directory,Duration::from_secs(600),cancel)?;
            query(binding,cancel)
        });
    }
    fn start(&mut self,ctx:egui::Context,work:impl FnOnce(&AtomicBool)->Result<Snapshot,String>+Send+'static) {
        self.cancel();self.loading=true;self.message=None;
        let cancel=Arc::new(AtomicBool::new(false));self.cancel=Some(cancel.clone());
        let (tx,rx)=mpsc::channel();self.receiver=Some(rx);
        std::thread::spawn(move ||{let result=work(&cancel);if !cancel.load(Ordering::Relaxed){let _=tx.send(result);ctx.request_repaint();}});
    }
    pub fn poll(&mut self) {
        if let Some(result)=self.receiver.as_ref().and_then(|rx|rx.try_recv().ok()) {
            self.receiver=None;self.cancel=None;self.loading=false;
            match result {Ok(snapshot)=>{self.snapshot=Some(snapshot);self.message=None;},Err(error)=>self.message=Some(error)}
        }
    }
    pub fn entries(&self)->Vec<crate::palette::Entry> {
        use crate::palette::{Entry,Target};
        let mut result=Vec::new();
        let mut add=|label:String,detail:String,action:Action|result.push(Entry{label,detail,shortcut:"",target:Target::Compose(action)});
        add("← Toutes les commandes".into(),"Revenir à la palette".into(),Action::Back);
        if self.loading {return result;}
        add("Actualiser Docker Compose".into(),"Interroger le moteur maintenant".into(),Action::Refresh);
        add("Choisir les fichiers Compose…".into(),"Auto, ou chemins de fichiers personnalisés".into(),Action::Configure);
        if self.message.is_some(){return result;}
        if let Some(s)=&self.snapshot {
            let detail=s.binding.describe();
            add("Démarrer le projet · up -d".into(),detail.clone(),Action::Up);
            add("Arrêter le projet · stop".into(),detail.clone(),Action::Stop);
            for service in &s.services {
                let containers:Vec<_>=s.containers.iter().filter(|c|&c.service==service).collect();
                let state=if containers.is_empty(){"non créé".into()}else{containers.iter().map(|c|format!("{} {}",c.state,c.health)).collect::<Vec<_>>().join(", ")};
                add(format!("Logs · {service}"),format!("{state} · {detail}"),Action::Logs(service.clone()));
                if !containers.is_empty(){add(format!("Redémarrer · {service}"),format!("{state} · {detail}"),Action::Restart(service.clone()));}
                for c in containers.into_iter().filter(|c|c.state=="running" && !c.id.is_empty()) {
                    add(format!("Shell · {} · {}",service,c.name),format!("{} · {detail}",c.health),Action::Shell(c.clone()));
                }
            }
        }
        result
    }
    pub fn subtitle(&self)->String {
        if self.loading {return "Docker Compose : opération en cours… (Échap annule l’attente)".into();}
        if let Some(error)=&self.message {return error.clone();}
        self.snapshot.as_ref().map(|s|format!("{} · instantané il y a {} s",s.binding.describe(),s.taken.elapsed().as_secs())).unwrap_or_default()
    }
}
impl Drop for Browser {fn drop(&mut self){self.cancel();}}

fn docker_path()->Result<PathBuf,String> {
    let path=std::env::var_os("PATH").unwrap_or_default();
    let candidates=std::env::split_paths(&path).map(|p|p.join("docker"))
        .chain(["/Applications/Docker.app/Contents/Resources/bin/docker","/opt/homebrew/bin/docker","/usr/local/bin/docker"].map(PathBuf::from));
    candidates.into_iter().find(|p|p.is_file()).ok_or_else(||"Docker introuvable. Installe Docker puis réessaie.".into())
}
fn resolve(root:&Path,files:Vec<PathBuf>,cancel:&AtomicBool)->Result<Binding,String> {
    let docker=docker_path()?;
    if files.is_empty() && std::env::var_os("COMPOSE_FILE").is_none_or(|s|s.is_empty())
        && !["compose.yaml","compose.yml","docker-compose.yaml","docker-compose.yml"].iter().any(|f|root.join(f).is_file()) {
        return Err("Aucun fichier Compose à la racine. Choisis les fichiers Compose de ce projet.".into());
    }
    let context_env=std::env::var("DOCKER_CONTEXT").ok().filter(|v|!v.is_empty());
    let host_env=std::env::var("DOCKER_HOST").ok().filter(|v|!v.is_empty());
    let (target,context,endpoint)=if context_env.is_none() && host_env.is_some() {
        let host=host_env.unwrap();(vec!["--host".into(),host.clone()],"DOCKER_HOST".into(),host)
    }else {
        let name=match context_env {Some(name)=>name,None=>run(&docker,&["context".into(),"show".into()],root,Duration::from_secs(8),cancel)?.trim().to_string()};
        let output=run(&docker,&["context".into(),"inspect".into(),name.clone()],root,Duration::from_secs(8),cancel)?;
        let json:serde_json::Value=serde_json::from_str(&output).map_err(|e|format!("Contexte Docker invalide : {e}"))?;
        let endpoint=json[0]["Endpoints"]["docker"]["Host"].as_str().unwrap_or("").to_string();
        (vec!["--context".into(),name.clone()],name,endpoint)
    };
    let files=files.into_iter().map(|f|if f.is_absolute(){f}else{root.join(f)}).collect();
    Ok(Binding{directory:root.to_owned(),docker,target,machine:Machine::from_endpoint(&endpoint),context,files})
}
fn query(binding:Binding,cancel:&AtomicBool)->Result<Snapshot,String> {
    let services=run(&binding.docker,&binding.compose_args(&["config","--services"]),&binding.directory,Duration::from_secs(8),cancel)?;
    let output=run(&binding.docker,&binding.compose_args(&["ps","--all","--format","json"]),&binding.directory,Duration::from_secs(8),cancel)?;
    let containers=parse_containers(&output)?;
    Ok(Snapshot{binding,services:services.lines().map(str::trim).filter(|s|!s.is_empty()).map(str::to_owned).collect(),containers,taken:Instant::now()})
}
fn parse_containers(text:&str)->Result<Vec<Container>,String> {
    let text=text.trim();if text.is_empty(){return Ok(vec![])}
    if text.starts_with('[') {serde_json::from_str(text).map_err(|e|format!("Réponse Docker invalide : {e}"))}
    else {text.lines().map(|line|serde_json::from_str(line).map_err(|e|format!("Réponse Docker invalide : {e}"))).collect()}
}

// Only this finite, user-initiated subprocess has a timed wait. Never polls Docker at rest.
fn run(binary:&Path,args:&[String],root:&Path,timeout:Duration,cancel:&AtomicBool)->Result<String,String> {
    use std::os::unix::process::CommandExt;
    let mut child=Command::new(binary).args(args).current_dir(root).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .process_group(0).spawn().map_err(|e|format!("Impossible de lancer Docker : {e}"))?;
    let finished=Arc::new(AtomicBool::new(false));
    fn reader(mut pipe:impl Read+std::os::fd::AsRawFd+Send+'static,finished:Arc<AtomicBool>)->std::thread::JoinHandle<(Vec<u8>,bool)> {
        unsafe {let flags=libc::fcntl(pipe.as_raw_fd(),libc::F_GETFL);libc::fcntl(pipe.as_raw_fd(),libc::F_SETFL,flags|libc::O_NONBLOCK);}
        std::thread::spawn(move ||{let mut out=Vec::new();let mut buf=[0;8192];let mut overflow=false;
            loop {match pipe.read(&mut buf) {
                Ok(0)=>break,
                Ok(n)=>{let remaining=1_048_576usize.saturating_sub(out.len());out.extend_from_slice(&buf[..n.min(remaining)]);overflow|=n>remaining;},
                Err(e) if e.kind()==std::io::ErrorKind::WouldBlock=>{if finished.load(Ordering::Relaxed){break;}std::thread::sleep(Duration::from_millis(5));},
                Err(e) if e.kind()==std::io::ErrorKind::Interrupted=>continue,
                Err(_)=>break,
            }}(out,overflow)})
    }
    let stdout=reader(child.stdout.take().unwrap(),finished.clone());let stderr=reader(child.stderr.take().unwrap(),finished.clone());
    let start=Instant::now();let mut interrupted=false;
    let status=loop {
        match child.try_wait(){Ok(Some(status))=>break Some(status),Err(_)=>{interrupted=true;break None},_=>{}}
        if cancel.load(Ordering::Relaxed)||start.elapsed()>timeout {interrupted=true;break None;}
        std::thread::sleep(Duration::from_millis(20));
    };
    if interrupted {unsafe{libc::kill(-(child.id() as i32),libc::SIGKILL);}let _=child.wait();}
    finished.store(true,Ordering::Relaxed);
    let (out,out_big)=stdout.join().unwrap_or_default();let (err,err_big)=stderr.join().unwrap_or_default();
    if interrupted{return Err("Docker : attente annulée ou délai dépassé. Une opération déjà transmise au moteur peut continuer ; actualise son état.".into());}
    if out_big||err_big{return Err("Réponse Docker trop volumineuse (limite 1 Mio).".into());}
    if !status.is_some_and(|s|s.success()){return Err(format!("Docker : {}",String::from_utf8_lossy(&err).chars().take(1500).collect::<String>()));}
    Ok(String::from_utf8_lossy(&out).into_owned())
}

pub fn terminal_args(binding:&Binding,action:&Action)->Option<(Vec<String>,Badge)> {
    let (args,label)=match action {
        Action::Shell(c)=>{let mut args=binding.target.clone();args.extend(["exec".into(),"-it".into(),c.id.clone(),"sh".into()]);(args,format!("DOCKER · {}",c.name))},
        Action::Logs(service)=>(binding.compose_args(&["logs","--follow","--tail","200","--",service]),format!("LOGS · {service}")),
        _=>return None,
    };
    Some((args,Badge{machine:binding.machine.clone(),label}))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn binding()->Binding {Binding{directory:"/tmp/project with spaces".into(),docker:"/usr/bin/docker".into(),target:vec!["--context".into(),"prod".into()],machine:Machine::Ssh("user@server".into()),context:"prod".into(),files:vec!["/tmp/project with spaces/compose.yaml".into()]}}
    #[test]
    fn relay_requires_token_and_keeps_machine_separate() {
        let local = parse_relay("ATELIER_DOCKER:secret:exec:unix:///docker.sock|wordpress-1", "secret").unwrap();
        assert_eq!(local.machine, Machine::Local);
        assert_eq!(local.label, "DOCKER · wordpress-1");
        let remote = parse_relay("ATELIER_DOCKER:secret:exec:ssh://user@host|php", "secret").unwrap();
        assert_eq!(remote.machine, Machine::Ssh("user@host".into()));
        assert!(parse_relay("ATELIER_DOCKER:other:exec:unix:///docker.sock|php", "secret").is_none());
        assert!(parse_relay("ATELIER_DOCKER:secret:exec:unix:///docker.sock|", "secret").is_none());
        assert!(parse_relay("docker exec -it php sh", "secret").is_none());
    }
    #[test]
    fn machine_and_container_are_independent_and_remote_tcp_never_local() {
        assert_eq!(Machine::from_endpoint("unix:///var/run/docker.sock"),Machine::Local);
        assert_eq!(Machine::from_endpoint("ssh://user@server"),Machine::Ssh("user@server".into()));
        assert!(matches!(Machine::from_endpoint("tcp://localhost:2375"),Machine::Remote(_)));
        assert_eq!(Machine::from_endpoint(""),Machine::Unknown);
        let c=Container{id:"abc".into(),name:"project-php-1".into(),service:"php".into(),state:"running".into(),health:"".into()};
        let (args,badge)=terminal_args(&binding(),&Action::Shell(c)).unwrap();
        assert_eq!(args,vec!["--context","prod","exec","-it","abc","sh"]);
        assert_eq!(badge.machine,Machine::Ssh("user@server".into()));
        assert_eq!(badge.label,"DOCKER · project-php-1");
    }
    #[test]
    fn compose_json_formats_and_service_arguments() {
        let row=r#"{"ID":"abc","Service":"php","Name":"app-php-1","State":"running","Health":"healthy"}"#;
        assert_eq!(parse_containers(&format!("[{row}]")).unwrap()[0].service,"php");
        assert_eq!(parse_containers(&format!("{row}\n{row}\n")).unwrap().len(),2);
        assert!(parse_containers("").unwrap().is_empty());
        assert!(parse_containers("invalid").is_err());
        let args=binding().compose_args(&["restart","--","app; touch /tmp/never"]);
        assert_eq!(args.last().unwrap(),"app; touch /tmp/never");
        assert_eq!(args[4],"/tmp/project with spaces/compose.yaml");
    }
    #[test]
    fn finite_worker_handles_success_failure_timeout_and_cancellation() {
        let root=std::env::temp_dir();let no=AtomicBool::new(false);
        assert_eq!(run(Path::new("/bin/sh"),&["-c".into(),"printf result".into()],&root,Duration::from_secs(2),&no).unwrap(),"result");
        assert!(run(Path::new("/bin/sh"),&["-c".into(),"echo failed >&2; exit 1".into()],&root,Duration::from_secs(2),&no).unwrap_err().contains("failed"));
        let start=Instant::now();
        assert!(run(Path::new("/bin/sh"),&["-c".into(),"sleep 30".into()],&root,Duration::from_millis(50),&no).is_err());
        assert!(start.elapsed()<Duration::from_secs(2));
        assert!(run(Path::new("/bin/sh"),&["-c".into(),"sleep 30".into()],&root,Duration::from_secs(30),&AtomicBool::new(true)).is_err());
    }
    #[test]
    fn palette_lists_uncreated_services_and_only_running_container_shells() {
        let mut browser=Browser::default();
        browser.snapshot=Some(Snapshot{binding:binding(),services:vec!["php".into(),"db".into()],containers:parse_containers(r#"[{"ID":"a","Service":"php","State":"running"},{"ID":"b","Service":"db","State":"exited"}]"#).unwrap(),taken:Instant::now()});
        let entries=browser.entries();
        assert_eq!(entries.iter().filter(|e|matches!(e.target,crate::palette::Target::Compose(Action::Shell(_)))).count(),1);
        assert_eq!(entries.iter().filter(|e|matches!(e.target,crate::palette::Target::Compose(Action::Logs(_)))).count(),2);
        for _ in 0..20 {browser.poll();}
        assert!(browser.receiver.is_none());assert!(!browser.loading);
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    #[test]
    fn fake_cli_refreshes_after_action_and_then_stays_idle() {
        let dir=std::env::temp_dir().join(format!("atelier-compose-test-{}",crate::ssh_status::token()));std::fs::create_dir(&dir).unwrap();
        let fake=dir.join("docker");
        std::fs::write(&fake,r#"#!/bin/sh
printf 'request\n' >> calls
for arg do
 case "$arg" in
 config) printf 'php\ndb\n'; exit 0;;
 ps) if [ -f stopped ]; then state=exited; else state=running; fi
 printf '{"ID":"abc","Name":"project-php-1","Service":"php","State":"%s"}\n' "$state"; exit 0;;
 stop) touch stopped; exit 0;;
 esac
done
exit 2
"#).unwrap();std::fs::set_permissions(&fake,std::fs::Permissions::from_mode(0o700)).unwrap();
        let binding=Binding{directory:dir.clone(),docker:fake,target:vec!["--context".into(),"test".into()],machine:Machine::Local,context:"test".into(),files:vec![]};
        let snapshot=query(binding,&AtomicBool::new(false)).unwrap();
        assert_eq!(snapshot.containers[0].state,"running");assert_eq!(snapshot.services,vec!["php","db"]);
        let mut browser=Browser::default();browser.active=true;browser.snapshot=Some(snapshot);
        browser.mutate(Action::Stop,egui::Context::default());
        let end=Instant::now()+Duration::from_secs(3);
        while browser.loading && Instant::now()<end {browser.poll();std::thread::sleep(Duration::from_millis(5));}
        assert!(!browser.loading);assert!(browser.message.is_none());assert_eq!(browser.snapshot.as_ref().unwrap().containers[0].state,"exited");
        let calls=std::fs::read_to_string(dir.join("calls")).unwrap();assert_eq!(calls.lines().count(),5);
        for _ in 0..100 {browser.poll();browser.entries();}
        assert_eq!(std::fs::read_to_string(dir.join("calls")).unwrap(),calls);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn container_command_returns_to_local_shell_even_on_failure() {
        use std::io::Write;
        let dir=std::env::temp_dir().join(format!("atelier-compose-shell-{}",crate::ssh_status::token()));std::fs::create_dir(&dir).unwrap();
        let mut child=Command::new("/bin/zsh").args(["-f","-c",RETURN_TO_LOCAL_SCRIPT,"atelier","/bin/sh","-c","exit 7"])
            .env("ATELIER_SSH_TOKEN","testtoken").env("ZDOTDIR",&dir).current_dir(&dir)
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
        child.stdin.take().unwrap().write_all(b"print LOCAL_READY; exit\n").unwrap();
        let output=child.wait_with_output().unwrap();let text=String::from_utf8_lossy(&output.stdout);
        assert!(output.status.success());assert!(text.contains("ATELIER_DOCKER:testtoken:local"));assert!(text.contains("code 7"));assert!(text.contains("LOCAL_READY"));
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[cfg(test)]
mod persistence_tests {
    use super::*;
    #[test]
    fn custom_compose_files_survive_state_roundtrip_and_old_states_load() {
        let mut state=crate::persistence::State::default();
        state.compose_files.insert("/tmp/my project".into(),vec!["compose.yaml".into(),"docker/dev override.yaml".into()]);
        let restored:crate::persistence::State=serde_json::from_str(&serde_json::to_string(&state).unwrap()).unwrap();
        assert_eq!(state.compose_files,restored.compose_files);
        let old:crate::persistence::State=serde_json::from_str(r#"{"version":1,"projects":[]}"#).unwrap();
        assert!(old.compose_files.is_empty());
    }
}

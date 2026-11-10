mod native;
use tafo::*;
fn main() {
    objc2::rc::autoreleasepool(|_| {
        if let Err(error) = run() {
            eprintln!("{error}");
            std::process::exit(1);
        }
    });
}
fn run() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args == ["--runtime-library-path"] {
        println!(
            "{}",
            baml_bridge::loader::ensure_library_cached()
                .map_err(|e| e.to_string())?
                .display()
        );
        return Ok(());
    }
    if args.iter().any(|s| s == "--help") {
        println!("Tafo — Rust + Nightly BAML + JEV\n  --snapshot\n  --activity\n  --history\n  --predict --dry-run\n  --dry-run [--text TEXT]\n  --tap\n  --prompt\n  --listen\n  --status\n  --quit");
        return Ok(());
    }
    if args.iter().any(|s| s == "--activity") {
        native::control("activity")?;
        return Ok(());
    }
    if args.iter().any(|s| s == "--history") {
        println!(
            "{}",
            serde_json::to_string_pretty(&Activity::load(&storage_dir().join("activity.json")))
                .map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    if args.iter().any(|s| s == "--status") {
        println!("{}", native::control("status")?);
        return Ok(());
    }
    if args.iter().any(|s| s == "--quit") {
        let _ = native::control("quit")?;
        return Ok(());
    }
    let launch = args.is_empty() || args.iter().any(|s| s == "--prompt" || s == "--listen");
    if launch {
        if native::control(if args.iter().any(|s| s == "--listen") {
            "status"
        } else {
            "prompt"
        })
        .is_ok()
        {
            return Ok(());
        }
        return unsafe { native::run(!args.iter().any(|s| s == "--listen")) };
    }
    if args == ["--tap"] && native::control("tap").is_ok() {
        return Ok(());
    }
    initialize_runtime()?;
    let context = unsafe { native::snapshot(&History::load(&storage_dir().join("history.json"))) }?;
    if args.iter().any(|s| s == "--snapshot") {
        println!(
            "{}",
            serde_json::to_string_pretty(&context).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    let text = if let Some(index) = args.iter().position(|s| s == "--text") {
        args.get(index + 1).ok_or("--text needs a request")?.clone()
    } else {
        String::new()
    };
    let mut engine = Engine::live();
    let choice = if args.iter().any(|s| s == "--predict") {
        engine.predict(&context)?
    } else {
        engine.choose(&context, &text)?
    };
    if args.iter().any(|s| s == "--dry-run") {
        println!(
            "{}",
            serde_json::to_string_pretty(&choice).map_err(|e| e.to_string())?
        );
        Ok(())
    } else {
        unsafe { native::activate(choice.bundle_id.as_deref().ok_or(choice.reason.as_str())?) }
    }
}

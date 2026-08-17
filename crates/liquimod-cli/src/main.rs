use clap::{Parser, Subcommand};
use liquimod_core::deploy::Deployer;
use liquimod_core::library::Library;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "liquimod", about = "LiquiMod core verification CLI")]
struct Cli {
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand)]
enum Command {
    /// 初始化仓库目录
    Init { #[arg(long)] root: PathBuf },
    /// 扫描仓库与索引对账
    Scan { #[arg(long)] root: PathBuf },
    /// 复制外部文件夹入仓库
    Add {
        #[arg(long)] root: PathBuf,
        #[arg(long)] src: PathBuf,
        #[arg(long)] character: String,
        #[arg(long)] name: String,
    },
    /// 启用 mod（创建 junction）
    Enable { #[arg(long)] root: PathBuf, #[arg(long)] mods_dir: PathBuf, #[arg(long)] id: i64 },
    /// 禁用 mod（删除 junction）
    Disable { #[arg(long)] root: PathBuf, #[arg(long)] mods_dir: PathBuf, #[arg(long)] id: i64 },
    /// 崩溃恢复 + 全量对账
    Reconcile { #[arg(long)] root: PathBuf, #[arg(long)] mods_dir: PathBuf },
    /// 查看状态一致性
    Status { #[arg(long)] root: PathBuf, #[arg(long)] mods_dir: PathBuf },
}

fn main() {
    let cli = Cli::parse();
    let result = run(cli);
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> liquimod_core::error::Result<()> {
    match cli.cmd {
        Command::Init { root } => {
            Library::init(&root)?;
            println!("initialized library at {}", root.display());
        }
        Command::Scan { root } => {
            let lib = Library::open(&root)?;
            for m in lib.scan()? {
                println!("#{} [{}] {} enabled={}", m.id, m.character, m.name, m.enabled);
            }
        }
        Command::Add { root, src, character, name } => {
            let lib = Library::open(&root)?;
            let m = lib.add_folder(&src, &character, &name)?;
            println!("added #{} [{}] {}", m.id, m.character, m.name);
        }
        Command::Enable { root, mods_dir, id } => {
            let lib = Library::open(&root)?;
            Deployer::new(&lib, &mods_dir).enable(id)?;
            println!("enabled #{id}");
        }
        Command::Disable { root, mods_dir, id } => {
            let lib = Library::open(&root)?;
            Deployer::new(&lib, &mods_dir).disable(id)?;
            println!("disabled #{id}");
        }
        Command::Reconcile { root, mods_dir } => {
            let lib = Library::open(&root)?;
            let d = Deployer::new(&lib, &mods_dir);
            d.recover()?;
            d.reconcile()?;
            println!("reconciled");
        }
        Command::Status { root, mods_dir } => {
            let lib = Library::open(&root)?;
            for (m, ok) in Deployer::new(&lib, &mods_dir).status()? {
                let mark = if ok { "OK" } else { "DRIFT" };
                println!("#{} [{}] {} enabled={} [{}]", m.id, m.character, m.name, m.enabled, mark);
            }
        }
    }
    Ok(())
}

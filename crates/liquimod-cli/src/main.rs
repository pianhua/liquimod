use clap::{Parser, Subcommand};
use liquimod_core::archive::install::{install_archive, InstallOutcome};
use liquimod_core::deploy::Deployer;
use liquimod_core::error::LiquiModError;
use liquimod_core::library::Library;
use std::io::{self, IsTerminal, Write};
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
    Init {
        #[arg(long)]
        root: PathBuf,
    },
    List {
        #[arg(long)]
        root: PathBuf,
    },
    /// 扫描仓库与索引对账
    Scan {
        #[arg(long)]
        root: PathBuf,
    },
    Install {
        archive: PathBuf,
        #[arg(long)]
        root: PathBuf,
        #[arg(long)]
        password: Option<String>,
        #[arg(long, default_value = "Others")]
        character: String,
    },
    /// 复制外部文件夹入仓库
    Add {
        #[arg(long)]
        root: PathBuf,
        #[arg(long)]
        src: PathBuf,
        #[arg(long)]
        character: String,
        #[arg(long)]
        name: String,
    },
    /// 启用 mod（创建 junction）
    Enable {
        #[arg(long)]
        root: PathBuf,
        #[arg(long)]
        mods_dir: PathBuf,
        #[arg(long)]
        id: i64,
    },
    /// 禁用 mod（删除 junction）
    Disable {
        #[arg(long)]
        root: PathBuf,
        #[arg(long)]
        mods_dir: PathBuf,
        #[arg(long)]
        id: i64,
    },
    /// 崩溃恢复 + 全量对账
    Reconcile {
        #[arg(long)]
        root: PathBuf,
        #[arg(long)]
        mods_dir: PathBuf,
    },
    /// 查看状态一致性
    Status {
        #[arg(long)]
        root: PathBuf,
        #[arg(long)]
        mods_dir: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();
    let result = run(cli);
    match result {
        Ok(()) => {}
        Err(CliError::NeedsPassword) => {
            eprintln!("error: archive requires a password; pass --password <pw>");
            std::process::exit(2);
        }
        Err(CliError::Core(error)) => {
            eprintln!("error: {error}");
            std::process::exit(1);
        }
    }
}

enum CliError {
    Core(LiquiModError),
    NeedsPassword,
}

impl From<LiquiModError> for CliError {
    fn from(error: LiquiModError) -> Self {
        Self::Core(error)
    }
}

impl From<io::Error> for CliError {
    fn from(error: io::Error) -> Self {
        Self::Core(error.into())
    }
}

fn run(cli: Cli) -> std::result::Result<(), CliError> {
    match cli.cmd {
        Command::Init { root } => {
            Library::init(&root)?;
            println!("initialized library at {}", root.display());
        }
        Command::List { root } => {
            let lib = Library::open(&root)?;
            for m in lib.list()? {
                println!(
                    "#{} [{}] {} enabled={}",
                    m.id, m.character, m.name, m.enabled
                );
            }
        }
        Command::Scan { root } => {
            let lib = Library::open(&root)?;
            for m in lib.scan()? {
                println!(
                    "#{} [{}] {} enabled={}",
                    m.id, m.character, m.name, m.enabled
                );
            }
        }
        Command::Install {
            archive,
            root,
            password,
            character,
        } => install(&root, &archive, &character, password.as_deref())?,
        Command::Add {
            root,
            src,
            character,
            name,
        } => {
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
                println!(
                    "#{} [{}] {} enabled={} [{}]",
                    m.id, m.character, m.name, m.enabled, mark
                );
            }
        }
    }
    Ok(())
}

fn install(
    root: &std::path::Path,
    archive: &std::path::Path,
    character: &str,
    password: Option<&str>,
) -> std::result::Result<(), CliError> {
    let library = Library::open(root)?;
    let outcome = install_archive(&library.db, &library, archive, character, password)?;
    let InstallOutcome::NeedsPassword = outcome else {
        return print_installation(outcome);
    };

    if !io::stdin().is_terminal() {
        return Err(CliError::NeedsPassword);
    }

    eprint!("Password: ");
    io::stderr().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim_end_matches(&['\r', '\n'][..]);
    let outcome = install_archive(&library.db, &library, archive, character, Some(input))?;
    print_installation(outcome)
}

fn print_installation(outcome: InstallOutcome) -> std::result::Result<(), CliError> {
    match outcome {
        InstallOutcome::Installed {
            mod_id,
            name,
            warnings,
        } => {
            println!("Installed: {name} (id {mod_id})");
            for warning in warnings {
                println!("Warning: {warning}");
            }
            Ok(())
        }
        InstallOutcome::NeedsPassword => Err(CliError::NeedsPassword),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_defaults_to_others_character() {
        let cli = Cli::try_parse_from(["liquimod", "install", "archive.zip", "--root", "library"])
            .unwrap();

        let Command::Install {
            archive,
            root,
            password,
            character,
        } = cli.cmd
        else {
            panic!("expected install command");
        };

        assert_eq!(archive, PathBuf::from("archive.zip"));
        assert_eq!(root, PathBuf::from("library"));
        assert_eq!(password, None);
        assert_eq!(character, "Others");
    }
}

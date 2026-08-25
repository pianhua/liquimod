use clap::{Parser, Subcommand};
use liquimod_core::archive::install::{install_archive, InstallOutcome};
use liquimod_core::deploy::Deployer;
use liquimod_core::error::LiquiModError;
use liquimod_core::library::Library;
use std::io::{self, IsTerminal};
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
    /// 安装归档文件
    Install {
        /// 归档文件路径
        archive: PathBuf,
        /// 库根目录
        #[arg(long)]
        root: PathBuf,
        /// 密码（会进入 shell 历史；推荐省略此参数后交互输入）
        #[arg(long)]
        password: Option<String>,
        /// 角色，默认值为 Others
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
        Err(CliError::PasswordCancelled) => {
            eprintln!("error: password input cancelled");
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
    PasswordCancelled,
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

    let input = password_input(rpassword::prompt_password("Password: "))?;
    let outcome = install_archive(&library.db, &library, archive, character, Some(&input))?;
    print_installation(outcome)
}

fn password_input(input: io::Result<String>) -> std::result::Result<String, CliError> {
    match input {
        Ok(password) if !password.is_empty() => Ok(password),
        Ok(_) => Err(CliError::PasswordCancelled),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::UnexpectedEof | io::ErrorKind::Interrupted
            ) =>
        {
            Err(CliError::PasswordCancelled)
        }
        Err(error) => Err(error.into()),
    }
}

fn print_installation(outcome: InstallOutcome) -> std::result::Result<(), CliError> {
    match outcome {
        InstallOutcome::Installed {
            mod_id,
            name,
            character,
            warnings,
        } => {
            println!("Installed: {name} -> {character} (id {mod_id})");
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
    use clap::CommandFactory;

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

    #[test]
    fn empty_password_is_cancelled() {
        assert!(matches!(
            password_input(Ok(String::new())),
            Err(CliError::PasswordCancelled)
        ));
    }

    #[test]
    fn eof_password_is_cancelled() {
        let result = password_input(Err(io::Error::new(io::ErrorKind::UnexpectedEof, "eof")));
        assert!(matches!(result, Err(CliError::PasswordCancelled)));
    }

    #[test]
    fn real_password_io_error_is_reported() {
        let result = password_input(Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "permission denied",
        )));

        assert!(matches!(
            result,
            Err(CliError::Core(LiquiModError::Io(error)))
                if error.kind() == io::ErrorKind::PermissionDenied
        ));
    }

    #[test]
    fn install_help_describes_password_input() {
        let mut command = Cli::command();
        let help = command
            .find_subcommand_mut("install")
            .unwrap()
            .render_help()
            .to_string();

        assert!(help.contains("归档文件路径"));
        assert!(help.contains("--password"));
        assert!(help.contains("shell 历史"));
        assert!(help.contains("--character"));
    }
}

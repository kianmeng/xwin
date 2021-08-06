use anyhow::{Context as _, Error};
use camino::Utf8PathBuf as PathBuf;
use structopt::StructOpt;
use tracing_subscriber::filter::LevelFilter;

fn setup_logger(json: bool, log_level: LevelFilter) -> Result<(), Error> {
    let mut env_filter = tracing_subscriber::EnvFilter::from_default_env();

    // If a user specifies a log level, we assume it only pertains to cargo_fetcher,
    // if they want to trace other crates they can use the RUST_LOG env approach
    env_filter = env_filter.add_directive(format!("xwin={}", log_level).parse()?);

    let subscriber = tracing_subscriber::FmtSubscriber::builder().with_env_filter(env_filter);

    if json {
        tracing::subscriber::set_global_default(subscriber.json().finish())
            .context("failed to set default subscriber")?;
    } else {
        tracing::subscriber::set_global_default(subscriber.finish())
            .context("failed to set default subscriber")?;
    }

    Ok(())
}

#[derive(StructOpt)]
pub enum Command {
    /// Displays a summary of the packages that would be downloaded.
    ///
    /// Note that this is not a full list as the SDK uses MSI files for many
    /// packages, so they would need to be downloaded and inspected to determine
    /// which CAB files must also be downloaded to get the content needed.
    List,
    /// Downloads all the selected packages that aren't already present in
    /// the download cache
    Download,
    /// Unpacks all of the downloaded packages to disk
    Unpack,
    /// Fixes the packages to prune unneeded files and adds symlinks to address
    /// file casing issues and then packs the final artifacts into directories
    /// or tarballs
    Pack {
        /// The MSVCRT includes (non-redistributable) debug versions of the
        /// various libs that are generally uninteresting to keep for most usage
        #[structopt(long)]
        include_debug_libs: bool,
        /// The MSVCRT includes PDB (debug symbols) files for several of the
        /// libraries that are genrally uninteresting to keep for most usage
        #[structopt(long)]
        include_debug_symbols: bool,
        /// By default, symlinks are added to both the CRT and WindowsSDK to
        /// address casing issues in general usage. For example, if you are
        /// compiling C/C++ code that does `#include <windows.h>`, it will break
        /// on a case-sensitive file system, as the actual path in the WindowsSDK
        /// is `Windows.h`. This also applies even if the C/C++ you are compiling
        /// uses correct casing for all CRT/SDK includes, as the internal headers
        /// also use incorrect casing in most cases.
        #[structopt(long)]
        disable_symlinks: bool,
        /// By default, we convert the MS specific `x64`, `arm`, and `arm64`
        /// target architectures to the more canonical `x86_64`, `aarch`, and
        /// `aarch64` of LLVM etc when creating directories/names. Passing this
        /// flag will preserve the MS names for those targets.
        #[structopt(long)]
        preserve_ms_arch_notation: bool,
        /// The root output directory. Defaults to `./.xwin-cache/pack` if not
        /// specified.
        #[structopt(long)]
        output: Option<PathBuf>,
        // Splits the CRT and SDK into architecture and variant specific
        // directories. The shared headers in the CRT and SDK are duplicated
        // for each output so that each combination is self-contained.
        // #[structopt(long)]
        // isolated: bool,
    },
}

const ARCHES: &[&str] = &["x86", "x86_64", "aarch", "aarch64"];
const VARIANTS: &[&str] = &["desktop", "onecore", /*"store",*/ "spectre"];
const LOG_LEVELS: &[&str] = &["off", "error", "warn", "info", "debug", "trace"];

fn parse_level(s: &str) -> Result<LevelFilter, Error> {
    s.parse::<LevelFilter>()
        .map_err(|_| anyhow::anyhow!("failed to parse level '{}'", s))
}

#[derive(StructOpt)]
pub struct Args {
    /// Doesn't display prompt to accept the license
    #[structopt(long, env = "XWIN_ACCEPT_LICENSE")]
    accept_license: bool,
    /// The log level for messages, only log messages at or above the level will be emitted.
    #[structopt(
        short = "L",
        long = "log-level",
        default_value = "info",
        parse(try_from_str = parse_level),
        possible_values(LOG_LEVELS),
    )]
    level: LevelFilter,
    /// Output log messages as json
    #[structopt(long)]
    json: bool,
    /// If set, will use a temporary directory for all files used for creating
    /// the archive and deleted upon exit, otherwise, all downloaded files
    /// are kept in the `--cache-dir` won't be retrieved again
    #[structopt(long)]
    temp: bool,
    /// Specifies the cache directory used to persist downloaded items to disk.
    /// Defaults to `./.xwin-cache` if not specified.
    #[structopt(long)]
    cache_dir: Option<PathBuf>,
    /// The version to retrieve, can either be a major version of 15 or 16, or
    /// a "<major>.<minor>" version.
    #[structopt(long, default_value = "16")]
    version: String,
    /// The product channel to use.
    #[structopt(long, default_value = "release")]
    channel: String,
    /// The architectures to include
    #[structopt(
        long,
        possible_values(ARCHES),
        use_delimiter = true,
        default_value = "x86_64"
    )]
    arch: Vec<xwin::Arch>,
    /// The variants to include
    #[structopt(
        long,
        possible_values(VARIANTS),
        use_delimiter = true,
        default_value = "desktop"
    )]
    variant: Vec<xwin::Variant>,
    #[structopt(subcommand)]
    cmd: Command,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Error> {
    let args = Args::from_args();
    setup_logger(args.json, args.level)?;

    if !args.accept_license {
        // The license link is the same for every locale, but we should probably
        // retrieve it from the manifest in the future
        println!("Do you accept the license at https://go.microsoft.com/fwlink/?LinkId=2086102 (yes | no)?");

        let mut accept = String::new();
        std::io::stdin().read_line(&mut accept)?;

        match accept.trim() {
            "yes" => println!("license accepted!"),
            "no" => anyhow::bail!("license not accepted"),
            other => anyhow::bail!("unknown response to license request {}", other),
        }
    }

    let cwd = PathBuf::from_path_buf(std::env::current_dir().context("unable to retrieve cwd")?)
        .map_err(|pb| anyhow::anyhow!("cwd {} is not a valid utf-8 path", pb.display()))?;

    let ctx = if args.temp {
        xwin::Ctx::with_temp()?
    } else {
        let cache_dir = match args.cache_dir {
            Some(cd) => cd,
            None => cwd.join(".xwin-cache"),
        };
        xwin::Ctx::with_dir(cache_dir)?
    };

    let ctx = std::sync::Arc::new(ctx);

    let pkg_manifest = xwin::get_pkg_manifest(&ctx, &args.version, &args.channel).await?;

    let arches = args.arch.into_iter().fold(0, |acc, arch| acc | arch as u32);
    let variants = args
        .variant
        .into_iter()
        .fold(0, |acc, var| acc | var as u32);

    let pruned = xwin::prune_pkg_list(&pkg_manifest, arches, variants)?;
    let pkgs = &pkg_manifest.packages;

    match args.cmd {
        Command::List => {
            print_packages(&pruned);
        }
        Command::Download => xwin::download(ctx, pkgs, pruned).await?,
        Command::Unpack => {
            xwin::download(ctx.clone(), pkgs, pruned.clone()).await?;
            xwin::unpack(ctx, pruned).await?;
        }
        Command::Pack {
            include_debug_libs,
            include_debug_symbols,
            disable_symlinks,
            preserve_ms_arch_notation,
            output,
        } => {
            xwin::download(ctx.clone(), pkgs, pruned.clone()).await?;
            xwin::unpack(ctx.clone(), pruned.clone()).await?;

            let output = output.unwrap_or_else(|| ctx.work_dir.join("pack"));

            xwin::pack(
                ctx,
                xwin::PackConfig {
                    include_debug_libs,
                    include_debug_symbols,
                    disable_symlinks,
                    preserve_ms_arch_notation,
                    output,
                },
                pruned,
                arches,
                variants,
            )?;
        }
    }

    Ok(())
}

fn print_packages(payloads: &[xwin::Payload]) {
    use cli_table::{format::Justify, Cell, Style, Table};

    let (dl, install) = payloads.iter().fold((0, 0), |(dl, install), payload| {
        (
            dl + payload.size,
            install + payload.install_size.unwrap_or_default(),
        )
    });

    let totals = vec![
        "Total".cell().bold(true).justify(Justify::Right),
        "".cell(),
        "".cell(),
        indicatif::HumanBytes(dl).cell().bold(true),
        indicatif::HumanBytes(install).cell().bold(true),
    ];

    let table = payloads
        .iter()
        .map(|payload| {
            vec![
                payload.filename.clone().cell().justify(Justify::Right),
                payload
                    .target_arch
                    .map(|a| a.to_string())
                    .unwrap_or_default()
                    .cell(),
                payload
                    .variant
                    .map(|v| v.to_string())
                    .unwrap_or_default()
                    .cell(),
                indicatif::HumanBytes(payload.size).cell(),
                indicatif::HumanBytes(payload.install_size.unwrap_or_default()).cell(),
            ]
        })
        .chain(std::iter::once(totals))
        .collect::<Vec<_>>()
        .table()
        .title(vec![
            "Name".cell(),
            "Target".cell(),
            "Variant".cell(),
            "Download Size".cell(),
            "Install Size".cell(),
        ]);

    let _ = cli_table::print_stdout(table);
}

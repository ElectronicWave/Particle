use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::{
        OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
};

use anyhow::{Context, Result, anyhow, bail};

const DEFAULT_LOCALE: &str = "en-US";
const LOCALES_DIR: &str = "locales";
const LOCALE_EXTENSION: &str = "toml";
const LOCALE_NAME_KEY: &str = "locale.name";

static CATALOG: OnceLock<Catalog> = OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocaleOption {
    pub code: String,
    pub name: String,
}

struct Locale {
    code: String,
    name: String,
    messages: BTreeMap<String, String>,
}

struct Catalog {
    locales: Vec<Locale>,
    current: AtomicUsize,
    default: usize,
}

/// Loads built-in and extension locales, then selects the locale for this process.
///
/// An explicit preference from the launcher config wins over `PARTICLE_LANG`,
/// which in turn wins over the system locale.
pub fn init(preferred: Option<&str>) -> Result<()> {
    let mut translations = builtin_translations()?;
    load_extensions(&mut translations)?;
    let locales = build_locales(translations);
    let default = default_index(&locales);
    let current = select_index(requested_locale(preferred).as_deref(), &locales).unwrap_or(default);

    CATALOG
        .set(Catalog {
            locales,
            current: AtomicUsize::new(current),
            default,
        })
        .map_err(|_| anyhow!("i18n was initialized more than once"))?;
    Ok(())
}

/// Returns the translation for `key` in the selected locale.
pub fn translate(key: &str) -> std::borrow::Cow<'static, str> {
    catalog().lookup(key)
}

pub fn available_locales() -> Vec<LocaleOption> {
    catalog()
        .locales
        .iter()
        .map(|locale| LocaleOption {
            code: locale.code.clone(),
            name: locale.name.clone(),
        })
        .collect()
}

pub fn current_locale() -> String {
    catalog().current().code.clone()
}

/// Selects a locale by its exact tag or language prefix.
pub fn set_locale(code: &str) -> Result<()> {
    let catalog = catalog();
    let index = select_index(Some(code), &catalog.locales)
        .ok_or_else(|| anyhow!("unknown locale: {code}"))?;
    catalog.current.store(index, Ordering::Relaxed);
    Ok(())
}

fn catalog() -> &'static Catalog {
    CATALOG
        .get()
        .expect("i18n::init() must run before the first translation lookup")
}

impl Catalog {
    fn current(&self) -> &Locale {
        &self.locales[self.current.load(Ordering::Relaxed)]
    }

    fn lookup(&'static self, key: &str) -> std::borrow::Cow<'static, str> {
        let current = self.current();
        if let Some(value) = message(current, key) {
            return std::borrow::Cow::Borrowed(value);
        }

        let language = current.code.split('-').next().unwrap_or(&current.code);
        let matched = self.locales.iter().find(|locale| {
            locale.code != current.code
                && locale
                    .code
                    .split('-')
                    .next()
                    .is_some_and(|candidate| candidate.eq_ignore_ascii_case(language))
        });
        if let Some(value) = matched.and_then(|locale| message(locale, key)) {
            return std::borrow::Cow::Borrowed(value);
        }

        message(&self.locales[self.default], key)
            .map(std::borrow::Cow::Borrowed)
            .unwrap_or_else(|| std::borrow::Cow::Owned(key.to_owned()))
    }
}

fn message(locale: &'static Locale, key: &str) -> Option<&'static str> {
    locale.messages.get(key).map(String::as_str)
}

fn build_locales(translations: BTreeMap<String, BTreeMap<String, String>>) -> Vec<Locale> {
    translations
        .into_iter()
        .map(|(code, messages)| {
            let name = messages
                .get(LOCALE_NAME_KEY)
                .cloned()
                .unwrap_or_else(|| code.clone());
            Locale {
                code,
                name,
                messages,
            }
        })
        .collect()
}

fn builtin_translations() -> Result<BTreeMap<String, BTreeMap<String, String>>> {
    let mut translations = BTreeMap::new();
    for (locale, source) in [
        ("en-US", include_str!("../../locales/en-US.toml")),
        ("zh-CN", include_str!("../../locales/zh-CN.toml")),
    ] {
        translations.insert(locale.to_owned(), parse_locale(source, locale)?);
    }
    Ok(translations)
}

fn load_extensions(translations: &mut BTreeMap<String, BTreeMap<String, String>>) -> Result<()> {
    let Some(directory) = locales_directory()? else {
        return Ok(());
    };

    let entries = fs::read_dir(&directory)
        .with_context(|| format!("failed to read locales directory: {}", directory.display()))?;
    for entry in entries {
        let entry = entry.with_context(|| {
            format!("failed to read locales directory: {}", directory.display())
        })?;
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some(LOCALE_EXTENSION) {
            continue;
        }

        let locale = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .with_context(|| format!("locale file name is not valid UTF-8: {}", path.display()))?;
        let source = fs::read_to_string(&path)
            .with_context(|| format!("failed to read locale file: {}", path.display()))?;
        let messages = parse_locale(&source, &path.display().to_string())?;
        translations
            .entry(locale.to_owned())
            .or_default()
            .extend(messages);
    }
    Ok(())
}

fn parse_locale(source: &str, origin: &str) -> Result<BTreeMap<String, String>> {
    let table = toml::from_str::<toml::Table>(source)
        .with_context(|| format!("failed to parse locale: {origin}"))?;
    let mut messages = BTreeMap::new();
    flatten_table("", table, &mut messages)?;
    Ok(messages)
}

fn flatten_table(
    prefix: &str,
    table: toml::Table,
    messages: &mut BTreeMap<String, String>,
) -> Result<()> {
    for (key, value) in table {
        let path = if prefix.is_empty() {
            key
        } else {
            format!("{prefix}.{key}")
        };
        match value {
            toml::Value::String(value) => {
                messages.insert(path, value);
            }
            toml::Value::Table(table) => flatten_table(&path, table, messages)?,
            other => bail!("translation {path} must be a string or table, found {other}"),
        }
    }
    Ok(())
}

fn locales_directory() -> Result<Option<PathBuf>> {
    let executable = std::env::current_exe().context("failed to locate the executable")?;
    let parent = executable
        .parent()
        .context("executable has no parent directory")?;
    let directory = parent.join(LOCALES_DIR);
    Ok(directory.is_dir().then_some(directory))
}

fn requested_locale(preferred: Option<&str>) -> Option<String> {
    if let Some(preferred) = preferred.filter(|value| !value.is_empty()) {
        return Some(preferred.to_owned());
    }
    // `PARTICLE_LANG` accepts the same BCP-47 tags as the system locale.
    if let Ok(value) = std::env::var("PARTICLE_LANG") {
        if !value.is_empty() {
            return Some(value);
        }
    }
    sys_locale::get_locale()
}

fn select_index(requested: Option<&str>, locales: &[Locale]) -> Option<usize> {
    let requested = requested?;
    let normalized = normalize_locale(requested);

    if let Some(exact) = locales
        .iter()
        .position(|locale| locale.code.eq_ignore_ascii_case(&normalized))
    {
        return Some(exact);
    }

    let language = normalized.split('-').next().unwrap_or(&normalized);
    locales.iter().position(|locale| {
        locale
            .code
            .split('-')
            .next()
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(language))
    })
}

fn default_index(locales: &[Locale]) -> usize {
    locales
        .iter()
        .position(|locale| locale.code.eq_ignore_ascii_case(DEFAULT_LOCALE))
        .or_else(|| {
            locales
                .iter()
                .position(|locale| locale.code.to_ascii_lowercase().starts_with("en"))
        })
        .unwrap_or(0)
}

fn normalize_locale(locale: &str) -> String {
    locale
        .split(['.', '@'])
        .next()
        .unwrap_or(locale)
        .replace('_', "-")
}

#[macro_export]
macro_rules! t {
    ($key:expr) => {
        $crate::i18n::translate($key)
    };
    ($key:expr, $($name:ident = $value:expr),+ $(,)?) => {{
        let mut text = $crate::i18n::translate($key).into_owned();
        $(
            let placeholder = concat!("%{", stringify!($name), "}");
            text = text.replace(placeholder, &format!("{}", $value));
        )+
        let result: std::borrow::Cow<'static, str> = std::borrow::Cow::Owned(text);
        result
    }};
}

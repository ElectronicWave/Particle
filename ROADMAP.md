# Particle Roadmap

Particle is a lightweight launcher powered by Elemental. The roadmap focuses on reliable daily use first, then builds out content management.

## Done

- Microsoft, Yggdrasil, and offline account login
- Loopback authentication server for offline accounts
- Local skin import, apply, remove, and preview
- Version browsing, installation, and launch for vanilla and common loaders
- Java runtime detection and pinned paths
- Configurable download parallelism
- English and Chinese UI

## 0.1 — Daily Driver

Goal: launch vanilla and modded instances reliably without another launcher.

### Compliance

- [ ] First-run EULA acceptance gate before installing or launching
- [ ] Link to the official EULA and persist acceptance per user

### Launch Configuration

- [ ] Heap size (`-Xmx` / `-Xms`)
- [ ] Extra JVM arguments
- [ ] Extra game arguments
- [ ] Window size
- [ ] QuickPlay: join a server or load a world directly
- [ ] Global defaults with per-instance overrides

### Instances

- [ ] Rename
- [ ] Duplicate
- [ ] Open instance folder
- [ ] Verify and repair instance files
- [ ] Search and sort by last launched

### Downloads and Runtimes

- [ ] Download mirror selection (BMCLAPI and others)
- [ ] Proxy, timeout, and retry settings
- [ ] Automatic Java selection per Minecraft version
- [ ] Automatic Java runtime download and installation

### Logs and Diagnostics

- [ ] Built-in log viewer
- [ ] Exit code and common crash explanations
- [ ] Launch failure diagnostics

### Accounts and Skins

- [ ] Rename local skins
- [ ] Infer Classic / Slim when importing a skin
- [ ] Local capes
- [ ] Re-login flow for expired MSA tokens
- [ ] Third-party authentication switch (authlib-injector), off by default
- [ ] Warning and explicit confirmation before enabling third-party authentication

## 0.2 — Content and Mods

Goal: install, manage, and share modded instances.

### Content Directories

- [ ] List `mods`, `resourcepacks`, and `shaderpacks`
- [ ] Import content from local files
- [ ] Remove content
- [ ] Enable and disable content with per-instance state
- [ ] Show file size, version, and source

### Mod Ecosystem

- [ ] Modrinth search and download
- [ ] CurseForge search and download
- [ ] Dependency resolution and missing dependency warnings
- [ ] Mod conflict warnings
- [ ] Update checks for mods and modpacks

### Modpacks

- [ ] Import `.mrpack`
- [ ] Import CurseForge modpacks
- [ ] Import FTB modpacks
- [ ] Export an instance as a modpack

## 0.3 — Content Extras

- [ ] World management
- [ ] Server list management
- [ ] Screenshot management
- [ ] Import instances from other launchers
- [ ] Launcher self-update and version checks

## Deferred

- 3D skin rendering
- Hosting modpacks inside the launcher

# TLM - TruckersMP Launcher Manager

A method for launching TruckersMP on Linux. TLM allows for launching via a Steam compatibility tool whilst providing features like launcher auto-updates and Steam overlay support!

## Setup (Steam compatibility tool)

Auto installers for the Steam compatibility tool part of TLM are provided for the `Steam Deck`, `Flatpak`, `Snap` and `Native` versions of Steam. For any other type of setup you may need to manually download the TLM binary from the [GitHub Releases Page](https://github.com/3ventic/tlm/releases/latest) or with install it with cargo (`cargo install --git https://github.com/3ventic/tlm`). Most installs of TLM will be kept up to date automatically unless explicitly disabled.

### Installers

Run one of the following commands to install TLM as a Steam compatibility tool. What command you need to run depends on how you have Steam installed. **These scripts CANNOT and SHOULD NOT be run with sudo or root permissions.**

Steam (Native) & SteamOS:

```sh
sh -c "$(curl -fsSL https://raw.githubusercontent.com/3ventic/tlm/main/setup/install-native.sh)"
```

Steam (Flatpak):

Not yet supported

---

#### Experimental

Steam (Snap) **[Unsupported - may be broken on Wayland]**

Not yet supported

---

After the installer has finished, please follow these steps to use the compatibility tool:

- **Ensure you have launched the game(s) you intend to play using Proton 10.0 in singleplayer first**
- Switch back to gaming mode (if on Steam Deck) or restart your Steam client otherwise.
- Navigate to your library and select "Euro Truck Simulator 2" or "American Truck Simulator".
- Open the game properties menu and make sure the "Launch Options" field is empty.
- Switch to the "compatibility" tab and enable the "Force the use of a specific Steam Play compatibility tool" checkbox.
- From the box that appears select "TruckersMP [TLM]" (if this does not show, please make sure you properly restarted Steam).
- You can now launch the game as usual. TruckersMP Launcher will be automatically installed and run for you.

### Passing extra arguments or environment variables on startup (Advanced & Optional)

When using the compatibility tool you have the option to pass extra launch arguments in two ways.

1. (For Users): You can add any available launch-command flag via Steam's "Launch Options" settings. You shouldn't need to do this by default, however it may be necessary for debugging and troubleshooting purposes.

2. (For Developers): You can set `--extra-launch-args` & `--extra-env-vars` during the `install-steam-tool` command. These values will be passed to the launch command every time TLM is ran and will ensure users use these additional arguments by default without additional steps. This will allow you to override key behaviours of TLM. This is also the only way to set extra environment variables for the launcher itself.

More information on launch flags can be found by running `tlm launch --help` or [viewing the code (advanced)](https://github.com/3ventic/tlm/blob/main/src/commands/launch.rs#L68).

### Prelaunch/Postlaunch scripts (Advanced users)

When installed as a Steam compatibility tool, TLM supports running scripts before the launcher is started and after it has closed. When launched from Steam, TLM will look inside of the its compatibility tool directory for directories named `prelaunch.d` and `postlaunch.d` and will run all shell scripts contained within. These scripts have to be placed manually after installing TLM and are considered an experimental feature.

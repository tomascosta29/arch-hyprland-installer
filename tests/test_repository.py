import json
import re
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path

REPOSITORY_ROOT = Path(__file__).resolve().parents[1]


class InstallerTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.installer = (REPOSITORY_ROOT / "install.sh").read_text()

    def test_fstab_is_written_to_installed_system(self):
        self.assertIn("genfstab -U /mnt > /mnt/etc/fstab", self.installer)
        self.assertNotRegex(self.installer, r"genfstab[^\n]+/mnt/fstab(?:\s|$)")

    def test_chroot_heredocs_do_not_expand_live_environment(self):
        chroot_invocations = re.findall(
            r"arch-chroot.*?<<'CHROOT'",
            self.installer,
            flags=re.DOTALL,
        )
        self.assertEqual(len(chroot_invocations), 2)
        self.assertNotRegex(self.installer, r"arch-chroot.*?<<CHROOT")

    def test_single_choice_desktop_stack(self):
        expected = {
            "nautilus",
            "kitty",
            "firefox",
            "hyprlock",
            "hypridle",
            "dunst",
            "hyprpaper",
            "hyprpolkitagent",
            "hyprshutdown",
            "quickshell",
            "networkmanager",
            "bluez",
            "pacman-contrib",
            "rofi",
            "otf-font-awesome",
            "sddm",
        }
        for package in expected:
            self.assertRegex(self.installer, rf"\b{re.escape(package)}\b")

        rejected = {
            "alacritty",
            "thunar",
            "swaylock",
            "mako",
            "gammastep",
            "polkit-kde-agent",
            "rofi-wayland",
            "ttf-font-awesome",
            "chromium",
            "google-chrome",
            "brave",
            "epiphany",
            "waybar",
        }
        for package in rejected:
            self.assertNotRegex(self.installer, rf"\b{re.escape(package)}\b")

    def test_installer_offers_keyboard_and_clock_settings(self):
        self.assertIn("KEYBOARD_LAYOUT", self.installer)
        self.assertIn("CLOCK_FORMAT", self.installer)
        self.assertIn("desktop-settings", self.installer)

    def test_installer_offers_flavor_selection(self):
        self.assertIn('INSTALL_FLAVOR="full"', self.installer)
        self.assertIn("Installation flavor (full/light)", self.installer)

    def test_installer_remains_interactive(self):
        self.assertNotIn("COSTA_INSTALL_NONINTERACTIVE", self.installer)
        self.assertNotIn("COSTA_INSTALL_CONFIRM_DISK", self.installer)
        self.assertIn("Type the full device path", self.installer)

    def test_sddm_theme_is_installed(self):
        self.assertIn("dotfiles/sddm/costa", self.installer)
        self.assertIn("/etc/sddm.conf.d/costa.conf", self.installer)
        self.assertIn("firefox.desktop", self.installer)

    def test_amd_baseline_is_explicit(self):
        for package in (
            "amd-ucode",
            "mesa",
            "vulkan-radeon",
            "mesa-utils",
            "vulkan-tools",
            "libva-utils",
        ):
            self.assertRegex(self.installer, rf"\b{re.escape(package)}\b")

        for retired_package in ("libva-mesa-driver", "mesa-vdpau"):
            self.assertNotRegex(
                self.installer,
                rf"\b{re.escape(retired_package)}\b",
            )

    def test_common_workstation_packages_are_explicit(self):
        for package in (
            "openssh",
            "firewalld",
            "gvfs-mtp",
            "file-roller",
            "sound-theme-freedesktop",
            "man-db",
            "man-pages",
            "noto-fonts",
            "noto-fonts-emoji",
            "noto-fonts-cjk",
            "dosfstools",
        ):
            self.assertRegex(self.installer, rf"\b{re.escape(package)}\b")

        for excluded_package in ("cups", "sane", "flatpak"):
            self.assertNotRegex(
                self.installer,
                rf"\b{re.escape(excluded_package)}\b",
            )

    def test_installation_profiles_separate_vm_and_bare_metal_packages(self):
        bare_metal = re.search(
            r"bare-metal\)\s+PROFILE_PACKAGES=\((.*?)\)\s+;;",
            self.installer,
            flags=re.DOTALL,
        )
        vm = re.search(
            r"vm\)\s+PROFILE_PACKAGES=\((.*?)\)\s+;;",
            self.installer,
            flags=re.DOTALL,
        )
        self.assertIsNotNone(bare_metal)
        self.assertIsNotNone(vm)

        for package in ("fwupd", "smartmontools", "nvme-cli", "ntfs-3g"):
            self.assertRegex(bare_metal.group(1), rf"\b{re.escape(package)}\b")
            self.assertNotRegex(vm.group(1), rf"\b{re.escape(package)}\b")

        for package in ("qemu-guest-agent", "spice-vdagent"):
            self.assertRegex(vm.group(1), rf"\b{re.escape(package)}\b")
            self.assertNotRegex(bare_metal.group(1), rf"\b{re.escape(package)}\b")

    def test_dual_disk_boot_and_keyring_integration_remain_enabled(self):
        self.assertRegex(self.installer, r"\bos-prober\b")
        self.assertIn("GRUB_DISABLE_OS_PROBER=false", self.installer)
        self.assertIn("pam_gnome_keyring", self.installer)
        self.assertIn("firewalld.service", self.installer)

    def test_preflight_precedes_password_and_disk_confirmation(self):
        self.assertIn("preflight_installation", self.installer)
        preflight = self.installer.index("preflight_installation\n\nread -r -s -p")
        password = self.installer.index("Password for '${USERNAME}'")
        confirmation = self.installer.index("Type the full device path")
        self.assertLess(preflight, password)
        self.assertLess(password, confirmation)
        self.assertIn("geo.mirror.pkgbuild.com/core/os/x86_64/core.db", self.installer)

    def test_partition_paths_are_discovered_from_lsblk(self):
        self.assertIn("lsblk -lnpo NAME,PARTN", self.installer)
        self.assertIn('discover_partition "${DISK}" 1', self.installer)
        self.assertIn('discover_partition "${DISK}" 2', self.installer)
        self.assertNotIn("*nvme* | *mmcblk*", self.installer)

    def test_installer_seeds_deployment_ownership_manifest(self):
        self.assertIn('MANIFEST_FILE="${MANIFEST_DIR}/managed-files"', self.installer)
        self.assertIn("install_costa_utils", self.installer)
        self.assertIn("scripts/lib/costa-utils.sh", self.installer)
        helper = (REPOSITORY_ROOT / "scripts" / "lib" / "costa-utils.sh").read_text()
        self.assertIn(r"BIN\tcosta-utils", helper)
        self.assertIn(r"BIN\tqs-activity", self.installer)


class ThemeTests(unittest.TestCase):
    def test_every_theme_is_complete(self):
        required = {
            "colors.css",
            "colors.lua",
            "dunstrc",
            "gtk-4.0/gtk.css",
            "kitty-theme.conf",
            "lock.conf",
            "rofi-theme.rasi",
            "wallpaper.png",
        }
        themes_dir = REPOSITORY_ROOT / "dotfiles" / "themes"
        for theme in themes_dir.iterdir():
            if not theme.is_dir():
                continue
            missing = [relative for relative in required if not (theme / relative).is_file()]
            self.assertFalse(missing, f"{theme.name} is missing {missing}")
            self.assertFalse(
                (theme / "quickshell").exists(),
                f"{theme.name} must not ship a parallel quickshell palette",
            )

    def test_gtk_themes_define_libadwaita_surfaces(self):
        required_vars = {
            "--window-bg-color",
            "--window-fg-color",
            "--sidebar-bg-color",
            "--sidebar-fg-color",
            "--popover-bg-color",
            "--popover-fg-color",
            "--headerbar-bg-color",
            "--headerbar-fg-color",
            "--view-bg-color",
            "--view-fg-color",
            "--accent-bg-color",
            "--accent-fg-color",
        }
        themes_dir = REPOSITORY_ROOT / "dotfiles" / "themes"
        for gtk_css in themes_dir.glob("*/gtk-4.0/gtk.css"):
            content = gtk_css.read_text()
            missing = [name for name in required_vars if f"{name}:" not in content]
            self.assertFalse(missing, f"{gtk_css.parent.parent.name} GTK CSS lacks {missing}")
            self.assertIn("@define-color popover_fg_color", content)
            self.assertIn("@define-color sidebar_bg_color", content)

    def test_theme_select_forces_prefer_dark(self):
        selector = (REPOSITORY_ROOT / "dotfiles" / "scripts" / "theme-select").read_text()
        self.assertIn("gtk-application-prefer-dark-theme=1", selector)
        self.assertIn("color-scheme prefer-dark", selector)
        self.assertIn("apply_gtk_color_scheme", selector)

    def test_theme_select_greps_css_variables_safely(self):
        selector = (REPOSITORY_ROOT / "dotfiles" / "scripts" / "theme-select").read_text()
        self.assertIn('grep -Fq -- "${required_variable}:"', selector)
        self.assertNotRegex(
            selector,
            r'grep -Fq "\$\{required_variable\}:"',
        )

    def test_theme_select_validates_full_palette_color_set(self):
        selector = (REPOSITORY_ROOT / "dotfiles" / "scripts" / "theme-select").read_text()
        for variable in (
            "background",
            "background-alt1",
            "background-alt2",
            "background-alt3",
            "background-alt4",
            "foreground",
            "foreground-dim",
            "soft-blue",
            "soft-cyan",
            "soft-green",
            "soft-yellow",
            "soft-peach",
            "soft-lavender",
            "soft-red",
            "soft-grey",
        ):
            self.assertRegex(
                selector,
                rf"\b{re.escape(variable)}\b",
                f"theme-select must validate palette color '{variable}'",
            )

    def test_theme_select_publishes_stable_consumer_destinations(self):
        selector = (REPOSITORY_ROOT / "dotfiles" / "scripts" / "theme-select").read_text()
        destinations = (
            'hypr/current_colors.lua"',
            'hypr/current_lock.conf"',
            'hypr/current_wallpaper.png"',
            'quickshell/costa/colors.css"',
            'kitty/kitty-theme.conf"',
            'rofi/rofi-theme.rasi"',
            'dunst/dunstrc"',
            'gtk-4.0/gtk.css"',
        )
        for destination in destinations:
            self.assertIn(
                destination,
                selector,
                f"theme-select must publish {destination.rstrip(chr(34))}",
            )

    def test_theme_palette_variables_exist(self):
        required_variables = {
            "background",
            "background-alt1",
            "background-alt2",
            "background-alt3",
            "background-alt4",
            "foreground",
            "foreground-dim",
            "soft-blue",
            "soft-cyan",
            "soft-green",
            "soft-yellow",
            "soft-peach",
            "soft-lavender",
            "soft-red",
            "soft-grey",
        }
        themes_dir = REPOSITORY_ROOT / "dotfiles" / "themes"
        for colors_file in themes_dir.glob("*/colors.css"):
            content = colors_file.read_text()
            defined = set(re.findall(r"@define-color\s+([\w-]+)", content))
            self.assertFalse(
                required_variables - defined,
                f"{colors_file.parent.name} lacks palette variables",
            )

    def test_quickshell_colors_consume_css_palette(self):
        colors = (REPOSITORY_ROOT / "dotfiles" / "quickshell" / "costa" / "Colors.qml").read_text()
        self.assertIn("colors.css", colors)
        self.assertIn("applyCss", colors)
        for name in (
            "background",
            "background-alt1",
            "foreground-dim",
            "soft-blue",
            "soft-cyan",
            "soft-lavender",
            "soft-red",
            "soft-yellow",
        ):
            self.assertIn(f'"{name}"', colors)
        self.assertIn("readonly property color accent: softBlue", colors)
        self.assertTrue(
            (REPOSITORY_ROOT / "dotfiles" / "quickshell" / "costa" / "colors.css").is_file()
        )


class ConfigurationTests(unittest.TestCase):
    @staticmethod
    def load_json(path):
        return json.loads(path.read_text())

    def test_quickshell_profile_json_is_valid(self):
        qs_dir = REPOSITORY_ROOT / "dotfiles" / "quickshell" / "costa"
        for filename in (
            "profile.json",
            "profile-bare-metal.json",
            "profile-vm.json",
            "user.json",
        ):
            parsed = self.load_json(qs_dir / filename)
            self.assertIsInstance(parsed, dict)

    def test_quickshell_shell_entrypoint_exists(self):
        qs_dir = REPOSITORY_ROOT / "dotfiles" / "quickshell" / "costa"
        for required in (
            "shell.qml",
            "Bar.qml",
            "Colors.qml",
            "colors.css",
            "Profile.qml",
            "CenterStage.qml",
            "AdaptiveTelemetry.qml",
        ):
            self.assertTrue((qs_dir / required).is_file(), required)

    def test_quickshell_and_hypr_costa_flags_resolve(self):
        consumers = (
            (REPOSITORY_ROOT / "dotfiles" / "quickshell" / "costa" / "Bar.qml").read_text(),
            (REPOSITORY_ROOT / "dotfiles" / "hypr" / "hyprland.lua").read_text(),
        )
        flags = set()
        for content in consumers:
            flags.update(re.findall(r'root\.costa,\s*"(--[a-z0-9-]+)"', content))
            flags.update(re.findall(r"costa-utils\s+(--[a-z0-9-]+)", content))
        self.assertTrue(flags, "expected costa-utils flags in Quickshell/Hyprland")
        target_rs = (
            REPOSITORY_ROOT / "costa-utils" / "crates" / "costa-core" / "src" / "target.rs"
        ).read_text()
        for flag in sorted(flags):
            self.assertIn(
                flag,
                target_rs,
                f"{flag} referenced by Quickshell/Hyprland is not in the Rust CLI surface",
            )

    def test_quickshell_workspaces_are_numbered(self):
        bar = (REPOSITORY_ROOT / "dotfiles" / "quickshell" / "costa" / "Bar.qml").read_text()
        self.assertIn("ids: root.primary ? [1, 2, 3, 4] : [5, 6, 7, 8]", bar)

    def test_hyprlock_sources_theme_lock_colors(self):
        lock = (REPOSITORY_ROOT / "dotfiles" / "hypr" / "hyprlock.conf").read_text()
        self.assertIn("source = ~/.config/hypr/current_lock.conf", lock)
        self.assertIn("path = $lock_wallpaper", lock)

    def test_theme_select_uses_current_hyprpaper_syntax(self):
        selector = (REPOSITORY_ROOT / "dotfiles" / "scripts" / "theme-select").read_text()
        self.assertIn("wallpaper {", selector)
        self.assertIn("fit_mode = cover", selector)
        self.assertNotIn("preload =", selector)

    def test_quickshell_uses_profile_for_primary_monitor(self):
        bar = (REPOSITORY_ROOT / "dotfiles" / "quickshell" / "costa" / "Bar.qml").read_text()
        self.assertIn("Profile.isPrimary(modelData.name)", bar)
        self.assertNotIn('modelData.name === "HDMI-A-1"', bar)

    def test_vm_profile_enables_virtio_3d(self):
        creator = (REPOSITORY_ROOT / "scripts" / "create-vm").read_text()
        self.assertIn("--graphics egl-headless", creator)
        self.assertIn("--graphics spice,listen=none", creator)
        self.assertNotIn("gl.enable=yes", creator)
        self.assertIn("model.acceleration.accel3d=yes", creator)

    def test_sddm_theme_metadata_exists(self):
        theme_dir = REPOSITORY_ROOT / "dotfiles" / "sddm" / "costa"
        self.assertTrue((theme_dir / "Main.qml").is_file())
        self.assertTrue((theme_dir / "metadata.desktop").is_file())
        self.assertTrue((theme_dir / "theme.conf").is_file())
        qml = (theme_dir / "Main.qml").read_text()
        self.assertRegex(qml, r"TextField\s*\{\s*id:\s*userField")
        self.assertIn("placeholderTextColor:", qml)
        self.assertIn("property date now:", qml)
        self.assertIn("Timer {", qml)
        self.assertIn("Screen.width", qml)
        self.assertIn("Screen.height", qml)
        conf = (REPOSITORY_ROOT / "dotfiles" / "sddm" / "costa.conf").read_text()
        self.assertIn("Current=costa", conf)

    def test_hyprland_exits_via_hyprshutdown(self):
        lua = (REPOSITORY_ROOT / "dotfiles" / "hypr" / "hyprland.lua").read_text()
        self.assertIn("hyprshutdown", lua)
        self.assertNotRegex(lua, r"hl\.bind\([^)\n]*hl\.dsp\.exit\(\)")

    def test_hyprland_configuration_is_lua_only(self):
        hypr_dir = REPOSITORY_ROOT / "dotfiles" / "hypr"
        themes_dir = REPOSITORY_ROOT / "dotfiles" / "themes"
        unexpected_hyprlang = set(hypr_dir.rglob("*.conf")) - {
            hypr_dir / "current_lock.conf",
            hypr_dir / "hypridle.conf",
            hypr_dir / "hyprlock.conf",
            hypr_dir / "hyprsunset.conf",
        }
        self.assertFalse(unexpected_hyprlang)
        self.assertFalse(list(themes_dir.glob("*/colors.conf")))

        for script_name in ("desktop-settings", "monitor-select", "theme-select"):
            script = (REPOSITORY_ROOT / "dotfiles" / "scripts" / script_name).read_text()
            self.assertNotIn("current_colors.conf", script)
            self.assertNotIn("input.conf", script)
            self.assertNotIn("monitors.conf", script)

    def test_hyprland_session_is_supervised_by_systemd(self):
        user_units = REPOSITORY_ROOT / "dotfiles" / "systemd" / "user"
        target = (user_units / "hyprland-session.target").read_text()
        lua = (REPOSITORY_ROOT / "dotfiles" / "hypr" / "hyprland.lua").read_text()

        self.assertIn("BindsTo=graphical-session.target", target)
        self.assertIn("Wants=xdg-desktop-autostart.target", target)
        self.assertIn("PropagatesStopTo=graphical-session.target", target)
        for unit in (
            "cliphist-image.service",
            "cliphist-text.service",
            "dunst.service",
            "hypridle.service",
            "hyprpaper.service",
            "hyprpolkitagent.service",
            "hyprsunset.service",
            "quickshell.service",
        ):
            self.assertIn(f"Wants={unit}", target)

        self.assertIn("systemctl --user start hyprland-session.target", lua)
        self.assertIn("systemctl --user stop hyprland-session.target", lua)
        self.assertIn("costa-quickshell-blur", lua)
        for direct_command in (
            'hl.exec_cmd("dunst")',
            'hl.exec_cmd("hypridle")',
            'hl.exec_cmd("hyprpaper")',
            'hl.exec_cmd("hyprsunset")',
            'hl.exec_cmd("waybar")',
            'hl.exec_cmd("quickshell")',
            'hl.exec_cmd("qs")',
            "wl-paste --type text --watch",
            "wl-paste --type image --watch",
        ):
            self.assertNotIn(direct_command, lua)

        for service in ("cliphist-image.service", "cliphist-text.service", "quickshell.service"):
            contents = (user_units / service).read_text()
            self.assertIn("PartOf=hyprland-session.target", contents)
            self.assertIn("Restart=on-failure", contents)

        for service in (
            "dunst",
            "hypridle",
            "hyprpaper",
            "hyprpolkitagent",
            "hyprsunset",
            "spice-vdagent",
        ):
            drop_in = (user_units / f"{service}.service.d" / "costa-session.conf").read_text()
            self.assertIn("PartOf=hyprland-session.target", drop_in)
            self.assertIn("After=hyprland-session.target", drop_in)
            self.assertIn("Restart=on-failure", drop_in)

        quickshell = (user_units / "quickshell.service").read_text()
        self.assertIn("ExecStart=/usr/bin/qs -c costa", quickshell)

    def test_check_updates_distinguishes_failures(self):
        script = (REPOSITORY_ROOT / "dotfiles" / "scripts" / "check_updates").read_text()
        self.assertIn('emit "0" "System up to date" "updated"', script)
        self.assertIn('"error"', script)
        self.assertIn("status=$?", script)
        self.assertNotIn("|| true", script)

    def test_runner_history_is_private(self):
        source = (
            REPOSITORY_ROOT
            / "costa-utils"
            / "crates"
            / "costa-core"
            / "src"
            / "backends"
            / "apps.rs"
        ).read_text()
        self.assertIn("0o600", source)
        self.assertIn("is_private_runner_command", source)
        self.assertIn("clear_runner_history", source)

    def test_window_capture_uses_json_geometry(self):
        source = (
            REPOSITORY_ROOT
            / "costa-utils"
            / "crates"
            / "costa-core"
            / "src"
            / "backends"
            / "blinker.rs"
        ).read_text()
        self.assertIn('["hyprctl", "-j", "activewindow"]', source)
        self.assertIn("No active window geometry", source)
        self.assertNotIn('re.search(r"at:', source)

    def test_screenshot_capture_plays_standard_shutter_sound(self):
        source = (
            REPOSITORY_ROOT
            / "costa-utils"
            / "crates"
            / "costa-core"
            / "src"
            / "backends"
            / "blinker.rs"
        ).read_text()
        self.assertIn('"canberra-gtk-play"', source)
        self.assertIn('"camera-shutter"', source)
        self.assertLess(
            source.index("if config.play_sound"), source.index("if config.show_notification")
        )

    def test_svg_icons_are_well_formed(self):
        icons_dir = REPOSITORY_ROOT / "costa-utils" / "assets" / "icons"
        for icon in icons_dir.glob("*.svg"):
            ET.parse(icon)

    def test_user_deployer_covers_every_config_component(self):
        deployer = (REPOSITORY_ROOT / "scripts" / "deploy-user").read_text()
        self.assertIn(
            "CONFIG_COMPONENTS=(dunst hypr kitty quickshell rofi scripts systemd themes)",
            deployer,
        )
        self.assertIn("install_costa_utils", deployer)
        self.assertIn("scripts/lib/costa-utils.sh", deployer)
        self.assertIn("dotfiles/mimeapps.list", deployer)
        self.assertIn("systemctl --user daemon-reload", deployer)
        self.assertIn('"${BIN_DIR}/costa-utils" --shutdown', deployer)
        self.assertIn("MANIFEST_FILE", deployer)
        self.assertIn("COSTA_DEPLOY_RELOAD", deployer)
        self.assertIn("remove_previous_manifest", deployer)
        self.assertIn("qs-activity", deployer)
        self.assertNotIn('pkill -f "${BIN_DIR}/costa-utils"', deployer)
        self.assertNotIn("costa_utils.py", deployer)

    def test_quickshell_vm_profile_omits_bare_metal_sensors(self):
        vm = self.load_json(
            REPOSITORY_ROOT / "dotfiles" / "quickshell" / "costa" / "profile-vm.json"
        )
        bare = self.load_json(
            REPOSITORY_ROOT / "dotfiles" / "quickshell" / "costa" / "profile-bare-metal.json"
        )
        self.assertFalse(vm["telemetry_gpu"])
        self.assertEqual(vm["role"], "single")
        self.assertTrue(bare["telemetry_gpu"])
        self.assertEqual(bare["role"], "dual-host")
        self.assertEqual(bare["primary_monitor"], "HDMI-A-1")
        telemetry = (
            REPOSITORY_ROOT / "dotfiles" / "quickshell" / "costa" / "AdaptiveTelemetry.qml"
        ).read_text()
        self.assertIn("Profile.telemetryGpu", telemetry)

    def test_theme_switch_is_a_single_pointer_transaction(self):
        selector = (REPOSITORY_ROOT / "dotfiles" / "scripts" / "theme-select").read_text()
        self.assertIn(
            'atomic_symlink "${TARGET_THEME}" "${THEME_STATE_DIR}/current-theme"', selector
        )
        self.assertIn("--shutdown", selector)
        self.assertNotIn("sync_sddm_theme", selector)

    def test_costa_utils_has_bounded_shared_backends(self):
        backends = REPOSITORY_ROOT / "costa-utils" / "crates" / "costa-core" / "src" / "backends"
        command = (
            REPOSITORY_ROOT / "costa-utils" / "crates" / "costa-core" / "src" / "command.rs"
        ).read_text()
        media = (backends / "media.rs").read_text()
        bluetooth = (backends / "bluetooth.rs").read_text()
        network = (backends / "network.rs").read_text()
        self.assertIn("wait_timeout", command)
        self.assertIn("Duration::from_secs(15)", command)
        self.assertIn("MAX_ARTWORK_BYTES", media)
        for operation in ("pair", "connect", "disconnect", "start_discovery"):
            self.assertIn(f"fn {operation}", bluetooth)
        for field in ("BSSID", "SECURITY", "IN-USE"):
            self.assertIn(field, network)

    def test_installed_vm_smoke_harness_exists(self):
        guest_validator = REPOSITORY_ROOT / "dotfiles" / "scripts" / "validate-installed"
        host_harness = REPOSITORY_ROOT / "scripts" / "vm-smoke"
        self.assertTrue(guest_validator.is_file())
        self.assertTrue(host_harness.is_file())
        self.assertIn("guest-exec-status", host_harness.read_text())

    def test_hyprland_uses_current_animation_leaf_names(self):
        config = (REPOSITORY_ROOT / "dotfiles" / "hypr" / "hyprland.lua").read_text()
        self.assertIn('leaf = "borderangle"', config)
        self.assertNotIn('leaf = "borderAngle"', config)

    def test_hyprland_uses_click_to_focus_for_reliable_popups(self):
        input_config = (REPOSITORY_ROOT / "dotfiles" / "hypr" / "input.lua").read_text()
        settings = (REPOSITORY_ROOT / "dotfiles" / "scripts" / "desktop-settings").read_text()
        self.assertIn("follow_mouse = 0", input_config)
        self.assertIn("follow_mouse = 0", settings)
        self.assertNotIn("follow_mouse = 1", input_config)
        self.assertNotIn("follow_mouse = 1", settings)

    def test_costa_utils_is_supervised_by_user_systemd(self):
        service = (
            REPOSITORY_ROOT / "dotfiles" / "systemd" / "user" / "costa-utils.service"
        ).read_text()
        target = (
            REPOSITORY_ROOT / "dotfiles" / "systemd" / "user" / "hyprland-session.target"
        ).read_text()
        cli = (
            REPOSITORY_ROOT / "costa-utils" / "crates" / "costa-core" / "src" / "target.rs"
        ).read_text()
        self.assertIn("ExecStart=%h/.local/bin/costa-utils --daemon", service)
        self.assertIn("SyslogIdentifier=costa-utils", service)
        self.assertIn("Wants=costa-utils.service", target)
        self.assertIn('"--daemon"', cli)

    def test_costa_utils_uses_gl_fallback_without_amdgpu(self):
        launcher = (
            REPOSITORY_ROOT / "costa-utils" / "crates" / "costa-ui" / "src" / "app.rs"
        ).read_text()
        self.assertIn('std::env::set_var("GSK_RENDERER", "cairo")', launcher)
        self.assertIn('std::env::set_var("GSK_RENDERER", "gl")', launcher)


if __name__ == "__main__":
    unittest.main()

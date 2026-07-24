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
        }
        for package in rejected:
            self.assertNotRegex(self.installer, rf"\b{re.escape(package)}\b")

    def test_installer_offers_keyboard_and_clock_settings(self):
        self.assertIn("KEYBOARD_LAYOUT", self.installer)
        self.assertIn("CLOCK_FORMAT", self.installer)
        self.assertIn("desktop-settings", self.installer)

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


class ThemeTests(unittest.TestCase):
    def test_every_theme_is_complete(self):
        required = {
            "colors.conf",
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

    def test_waybar_theme_variables_exist(self):
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
                f"{colors_file.parent.name} lacks Waybar variables",
            )


class ConfigurationTests(unittest.TestCase):
    @staticmethod
    def load_jsonc(path):
        content = "\n".join(line.split("//", 1)[0] for line in path.read_text().splitlines())
        return json.loads(content)

    def test_waybar_jsonc_is_valid(self):
        waybar_dir = REPOSITORY_ROOT / "dotfiles" / "waybar"
        for filename in ("config.jsonc", "modules", "user.jsonc"):
            parsed = self.load_jsonc(waybar_dir / filename)
            self.assertIsInstance(parsed, dict)

    def test_waybar_workspaces_are_numbered(self):
        modules = self.load_jsonc(REPOSITORY_ROOT / "dotfiles" / "waybar" / "modules")
        workspaces = modules["hyprland/workspaces"]
        self.assertEqual(workspaces["format"], "{id}")
        self.assertNotIn("format-icons", workspaces)

    def test_hyprlock_sources_theme_lock_colors(self):
        lock = (REPOSITORY_ROOT / "dotfiles" / "hypr" / "hyprlock.conf").read_text()
        self.assertIn("source = ~/.config/hypr/current_lock.conf", lock)
        self.assertIn("path = $lock_wallpaper", lock)

    def test_sddm_theme_metadata_exists(self):
        theme_dir = REPOSITORY_ROOT / "dotfiles" / "sddm" / "costa"
        self.assertTrue((theme_dir / "Main.qml").is_file())
        self.assertTrue((theme_dir / "metadata.desktop").is_file())
        self.assertTrue((theme_dir / "theme.conf").is_file())
        conf = (REPOSITORY_ROOT / "dotfiles" / "sddm" / "costa.conf").read_text()
        self.assertIn("Current=costa", conf)

    def test_svg_icons_are_well_formed(self):
        icons_dir = REPOSITORY_ROOT / "dotfiles" / "costa-utils" / "icons"
        for icon in icons_dir.glob("*.svg"):
            ET.parse(icon)

    def test_user_deployer_covers_every_config_component(self):
        deployer = (REPOSITORY_ROOT / "scripts" / "deploy-user").read_text()
        self.assertIn(
            "CONFIG_COMPONENTS=(dunst hypr kitty rofi scripts themes waybar)",
            deployer,
        )
        self.assertIn("dotfiles/costa-utils", deployer)
        self.assertIn('pkill -f "${BIN_DIR}/costa-utils"', deployer)

    def test_hyprland_uses_current_animation_leaf_names(self):
        config = (REPOSITORY_ROOT / "dotfiles" / "hypr" / "hyprland.lua").read_text()
        self.assertIn('leaf = "borderangle"', config)
        self.assertNotIn('leaf = "borderAngle"', config)

    def test_costa_utils_uses_gl_fallback_without_amdgpu(self):
        launcher = (REPOSITORY_ROOT / "dotfiles" / "costa-utils" / "costa_utils.py").read_text()
        self.assertIn('os.environ.setdefault("GSK_RENDERER", "cairo")', launcher)
        self.assertIn('os.environ.setdefault("GSK_RENDERER", "gl")', launcher)


if __name__ == "__main__":
    unittest.main()

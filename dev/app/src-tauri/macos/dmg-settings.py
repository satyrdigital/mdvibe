# dmgbuild settings for the MDVibe disk image (window layout without Finder
# scripting, so it works headless and in CI). Called by dev/scripts/package-release.mjs:
#   python3 -m dmgbuild -s dmg-settings.py -D app=<MDVibe.app> -D assets=<this folder> "MDVibe" out.dmg
import os.path

app = defines["app"]  # noqa: F821 (provided by dmgbuild)
here = defines["assets"]  # noqa: F821 (this folder)

format = "UDZO"
filesystem = "HFS+"
files = [app]
symlinks = {"Applications": "/Applications"}
badge_icon = None
icon = os.path.join(here, "..", "icons", "icon.icns")  # volume icon
icon_locations = {os.path.basename(app): (180, 210), "Applications": (480, 210)}

background = os.path.join(here, "dmg-background.tiff")
window_rect = ((200, 160), (660, 420))
default_view = "icon-view"
show_status_bar = False
show_tab_view = False
show_toolbar = False
show_pathbar = False
show_sidebar = False
icon_size = 112
text_size = 13
include_icon_view_settings = True

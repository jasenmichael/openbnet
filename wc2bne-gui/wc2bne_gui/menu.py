"""The desktop menu entry and icon, in the user's own data folder (XDG_DATA_HOME)."""
import os
import shutil

APP_ID = "io.github.jasenmichael.wc2bne"
HERE = os.path.dirname(os.path.abspath(__file__))


def places(env):
    """-> (entry path, icon path). Pure."""
    data = env.get("XDG_DATA_HOME") or os.path.join(env["HOME"], ".local", "share")
    return (os.path.join(data, "applications", APP_ID + ".desktop"),
            os.path.join(data, "icons", "hicolor", "scalable", "apps", APP_ID + ".svg"))


def quote(path):
    """A path as one argument of a desktop entry's Exec line. Pure."""
    for special in ("\\", '"', "`", "$"):
        path = path.replace(special, "\\" + special)
    return '"%s"' % path.replace("\\", "\\\\")


def entry(program):
    """-> the desktop entry's text. Pure."""
    return "\n".join([
        "[Desktop Entry]",
        "Type=Application",
        "Name=Warcraft II on Linux",
        "Comment=Install and play Warcraft II: Battle.net Edition",
        "Exec=" + quote(program),
        "Icon=" + APP_ID,
        "Terminal=false",
        "Categories=Game;StrategyGame;",
        "StartupNotify=true",
        "",
    ])


def remove(env):
    for path in places(env):
        if os.path.exists(path):
            os.remove(path)


def add(env, program):
    entry_path, icon_path = places(env)
    for path in (entry_path, icon_path):
        os.makedirs(os.path.dirname(path), exist_ok=True)
    shutil.copyfile(os.path.join(HERE, "icon.svg"), icon_path)
    with open(entry_path + ".new", "w", encoding="utf-8") as f:
        f.write(entry(program))
    os.replace(entry_path + ".new", entry_path)

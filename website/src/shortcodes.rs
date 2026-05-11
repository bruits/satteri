use maudit::content::shortcodes::{MarkdownShortcodes, ShortcodeArgs};
use maudit::route::PageContext;

const MANAGERS: &[Manager] = &[
    Manager {
        key: "pnpm",
        label: "pnpm",
        add: "pnpm add",
        dev_flag: "-D",
    },
    Manager {
        key: "npm",
        label: "npm",
        add: "npm install",
        dev_flag: "--save-dev",
    },
    Manager {
        key: "yarn",
        label: "yarn",
        add: "yarn add",
        dev_flag: "-D",
    },
    Manager {
        key: "bun",
        label: "bun",
        add: "bun add",
        dev_flag: "--dev",
    },
];

struct Manager {
    key: &'static str,
    label: &'static str,
    add: &'static str,
    dev_flag: &'static str,
}

pub fn register(shortcodes: &mut MarkdownShortcodes) {
    shortcodes.register("install", install);
}

fn install(args: &ShortcodeArgs, _ctx: Option<&mut PageContext>) -> String {
    let pkg = args.get_str_required("pkg").trim();
    let dev = args.get_or::<bool>("dev", false);

    let mut out = String::new();
    out.push_str("<div class=\"pkg-tabs\" data-pkg-tabs>\n");
    out.push_str("<div class=\"pkg-tabbar\" role=\"tablist\">\n");
    for (i, m) in MANAGERS.iter().enumerate() {
        let active = if i == 0 { " active" } else { "" };
        out.push_str(&format!(
            "<button type=\"button\" class=\"pkg-tab{active}\" data-pkg=\"{}\" role=\"tab\">{}</button>\n",
            m.key, m.label,
        ));
    }
    out.push_str("</div>\n");

    for (i, m) in MANAGERS.iter().enumerate() {
        let active = if i == 0 { " active" } else { "" };
        let flag = if dev {
            format!(" {}", m.dev_flag)
        } else {
            String::new()
        };
        out.push_str(&format!(
            "<div class=\"pkg-tab-panel{active}\" data-pkg=\"{}\" role=\"tabpanel\">\n\n```bash\n{}{} {}\n```\n\n</div>\n",
            m.key, m.add, flag, pkg,
        ));
    }
    out.push_str("</div>\n");
    out
}

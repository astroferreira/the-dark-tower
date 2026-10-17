//! The lab (`src/lab/`): the world, its regions, embarks and history as tools.
//!
//! planet_lab mcp                          serve the tools over MCP (stdio), for an agent
//! planet_lab tools                        list the tools and what they do
//! planet_lab call TOOL JSON [TOOL JSON]   run tools in one session, in order, e.g.
//!     planet_lab call open_world '{"dev":true}' places '{"x":42,"y":32,"kind":"cave"}'

use planet_generator::lab;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // The realistic world unless PLANET_MYTH=1 (one setting per process, as in the game).
    planet_generator::myth::set(planet_generator::myth::from_env());
    match args.first().map(String::as_str) {
        Some("mcp") => {
            if let Err(e) = lab::mcp::serve() { eprintln!("planet_lab mcp: {e}"); std::process::exit(1); }
        }
        Some("tools") => {
            for t in lab::tools() {
                println!("{}\n  {}\n  args: {}\n", t.name, t.description, serde_json::to_string(&(t.schema)()["properties"]).unwrap_or_default());
            }
        }
        Some("call") => {
            let rest = &args[1..];
            if rest.is_empty() || rest.len() % 2 != 0 { eprintln!("planet_lab call TOOL JSON [TOOL JSON ...]"); std::process::exit(2); }
            let mut session = lab::Session::new();
            let mut failed = false;
            for pair in rest.chunks(2) {
                let a: serde_json::Value = match serde_json::from_str(&pair[1]) {
                    Ok(v) => v,
                    Err(e) => { eprintln!("{}: bad JSON arguments: {e}", pair[0]); std::process::exit(2); }
                };
                println!("== {} {}", pair[0], pair[1]);
                match session.call(&pair[0], &a) {
                    Ok(out) => println!("{}\n", out.render()),
                    Err(e) => { println!("error: {e}\n"); failed = true; }
                }
            }
            if failed { std::process::exit(1); }
        }
        _ => {
            eprintln!("planet_lab mcp | tools | call TOOL JSON [TOOL JSON ...]");
            std::process::exit(2);
        }
    }
}

"""Gera docs/mcp.md a partir das ferramentas que o próprio MagoAgent anuncia (tools/list do `magoagent mcp`).
Uso: python docs/mcp.py [caminho do executável]   (padrão: target/release/magoagent[.exe])
O teste `every_tool_is_in_the_mcp_docs` (src/mcp/tests.rs) falha quando surge uma ferramenta que não está aqui."""
import json
import os
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
exe = sys.argv[1] if len(sys.argv) > 1 else os.path.join(ROOT, "target", "release", "magoagent.exe" if os.name == "nt" else "magoagent")

# uma cena vazia de teste: listar não toca no desenho de ninguém
scene = os.path.join(tempfile.mkdtemp(), "docs.mago")
with open(scene, "w", encoding="utf-8") as file:
    file.write('{"version":1,"elements":[],"background":[255,255,255,255]}')
process = subprocess.Popen([exe, "mcp"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, encoding="utf-8",
                           env=dict(os.environ, MAGOAGENT_SCENE=scene), creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))


def call(identifier, method, params):
    process.stdin.write(json.dumps({"jsonrpc": "2.0", "id": identifier, "method": method, "params": params}) + "\n")
    process.stdin.flush()
    while True:
        line = json.loads(process.stdout.readline())
        if line.get("id") == identifier:
            return line["result"]


info = call(1, "initialize", {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "docs", "version": "1"}})
tools = call(2, "tools/list", {})["tools"]
process.terminate()

GROUPS = [
    ("Canvas e desenho", ("add_elements", "update_elements", "delete_elements", "get_scene", "clear_canvas", "set_background", "view", "screenshot",
                          "focus_canvas", "canvas_drop", "canvas_group", "organize_canvas", "minimap", "sidebar", "panel", "quick_note", "changes_review", "theme", "window_outline", "history", "open_file", "save_file")),
    ("Notas e fichário", ("create_note", "read_note", "write_note", "note", "note_buttons", "note_stack", "binder", "open_as_note")),
    ("Agentes, terminais e a Luna", ("list_agents", "ask_agent", "recruit_agent", "create_terminal", "connect", "disconnect", "connection_history", "read_terminal",
                                      "terminal_find", "terminal_look", "terminal_menu", "terminal_processes", "terminal_restart", "notify",
                                      "ask_person", "remember", "review", "demands", "outside_message", "timeline", "tasks", "skills", "maestro_comment", "avatar",
                                      "avatar_follow", "flow", "usage", "usage_refresh", "memory")),
    ("Chats", ("chat_new", "chat_open", "chats_view", "chat_send", "chat_queue", "chat_list", "chat_search", "chat_rename", "chat_provider", "chat_command", "chat_context",
               "chat_decide", "chat_node", "mini_chat")),
    ("Árvore de Arquivos e editor", ("file_tree", "ide_open", "ide_search", "ide_folder", "open_path", "file_link", "open_default", "set_folder")),
    ("Navegador (portais)", ("create_portal", "portal_navigate", "portal_read", "portal_click", "portal_fill", "portal_eval", "portal_scroll",
                             "portal_wait", "portal_screenshot", "portal_copy_screenshot", "portal_console", "portal_history", "portal_tab",
                             "portal_settings", "vault_list", "vault_accounts", "vault_fill")),
    ("Janelas, espaços e andares", ("show_app", "show_screen", "resize_window", "close_window", "workspaces", "workspace_groups", "workspace_menu",
                                    "floors")),
    ("Ilha", ("island", "island_close_after", "island_color", "island_sound", "island_volume")),
    ("Ajustes, integrações, voz e app", ("set_setting", "dictionary", "email", "integrations", "voice", "voice_call", "voice_studio_url", "voice_studio_voice", "update", "tour",
                                         "docs", "loja", "magoagent")),
]


def describe(schema):
    properties = (schema or {}).get("properties", {})
    required = set((schema or {}).get("required", []))
    rows = []
    for name, spec in properties.items():
        kind = spec.get("type", "")
        if "enum" in spec:
            kind += ": " + ", ".join(f"`{value}`" for value in spec["enum"])
        text = spec.get("description", "").replace("\n", " ").replace("|", "\\|")
        rows.append(f"| `{name}`{' (obrigatório)' if name in required else ''} | {kind} | {text} |")
    return ["| Parâmetro | Tipo | O que é |", "|---|---|---|", *rows] if rows else ["Sem parâmetros."]


by_name = {tool["name"]: tool for tool in tools}
placed = set()
out = [
    "# O MCP do MagoAgent",
    "",
    "<!-- gerado por docs/mcp.py a partir do tools/list do app: não edite à mão -->",
    "",
    f"O servidor MCP vem no próprio executável (`magoagent mcp`, {info['serverInfo']['name']} {info['serverInfo']['version']}) e tem "
    f"**{len(tools)} ferramentas**. Ele fala com o app aberto pela ponte local (com token) e com o desenho pelo arquivo da cena; "
    "o que a IA desenha aparece traço a traço com o nome do agente.",
    "",
    "## Ligar",
    "",
    "- **Chats do app**: já recebem o MCP sozinhos (`--mcp-config` no Claude Code, `-c mcp_servers…` no Codex).",
    "- **Claude Code de fora**: `claude mcp add magoagent -- <caminho do magoagent> mcp`.",
    "- **Codex**: em `~/.codex/config.toml`, `[mcp_servers.magoagent]` com `command = \"<caminho do magoagent>\"` e `args = [\"mcp\"]`.",
    "- **opencode**: em `opencode.json`, `\"mcp\": { \"magoagent\": { \"type\": \"local\", \"command\": [\"<caminho do magoagent>\", \"mcp\"] } }`.",
    "- **Testar sem cliente**: mande JSON-RPC pelo stdin de `magoagent mcp` (`initialize` com `clientInfo.name`, `tools/list`, `tools/call`).",
    "",
    "Um terminal comum (que não é o da Luna) só mexe nas árvores ligadas a ele por corda; criar terminais, navegadores e conexões é "
    "da Luna e dos chats. O que uma ação abre para quem pediu (um navegador do `open_path`, por exemplo) fica ligado a ele por corda.",
    "",
]
for title, names in GROUPS:
    present = [name for name in names if name in by_name]
    if not present:
        continue
    out += [f"## {title}", ""]
    for name in present:
        placed.add(name)
        tool = by_name[name]
        out += [f"### `{name}`", "", tool.get("description", "").strip(), "", *describe(tool.get("inputSchema")), ""]
rest = [tool["name"] for tool in tools if tool["name"] not in placed]
if rest:
    out += ["## Outras", ""]
    for name in rest:
        tool = by_name[name]
        out += [f"### `{name}`", "", tool.get("description", "").strip(), "", *describe(tool.get("inputSchema")), ""]
with open(os.path.join(ROOT, "docs", "mcp.md"), "w", encoding="utf-8", newline="\n") as file:
    file.write("\n".join(out).rstrip() + "\n")
print(f"docs/mcp.md: {len(tools)} ferramentas ({len(rest)} em Outras)")

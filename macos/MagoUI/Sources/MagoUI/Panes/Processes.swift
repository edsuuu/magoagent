import SwiftUI

/// The processes under a terminal's shell (the footer's button; app/terminal/procs.rs): a heading with the icon, "Processos"
/// in semibold and a chip with how many and their memory; a row each, lit under the pointer, with its PID, its memory at the
/// right and a × that stops it. Read again every 3 s while open (`ui.terminal.processes`), in the menus' colors.
struct TerminalProcesses: View {
    let id: String

    private struct Answer: Decodable {
        var chip: String?
        var rows: [Row]
        var look: Look
    }

    fileprivate struct Row: Decodable, Identifiable {
        var pid: Int
        var name: String
        var memory: String
        var id: Int { pid }
    }

    fileprivate struct Look: Decodable {
        var bg: [Int]
        var border: [Int]
        var text: [Int]
        var muted: [Int]
        var hover: [Int]
    }

    @State private var answer: Answer?

    private static let width: CGFloat = 280

    var body: some View {
        VStack(spacing: 0) {
            if let answer {
                content(answer)
            } else {
                Color.clear.frame(height: 38)
            }
        }
        .frame(width: Self.width)
        .background { if let answer { Color(rgba: answer.look.bg) } }
        .task {
            while !Task.isCancelled {
                await load()
                try? await Task.sleep(for: .seconds(3))
            }
        }
    }

    @ViewBuilder private func content(_ answer: Answer) -> some View {
        let look = answer.look
        HStack(spacing: 0) {
            Image(systemName: "server.rack").font(.system(size: 11)).foregroundStyle(Color(rgba: look.muted)).frame(width: 13)
                .padding(.leading, 13.5)
            Text("Processos").font(.system(size: 13, weight: .semibold)).foregroundStyle(Color(rgba: look.text)).padding(.leading, 7.5)
            Spacer(minLength: 8)
            if let chip = answer.chip {
                Text(chip).font(.code(10.5)).foregroundStyle(Color(rgba: look.muted))
                    .padding(.horizontal, 7).frame(height: 20)
                    .background(Color(rgba: look.hover), in: Capsule())
                    .padding(.trailing, 12)
            }
        }
        .frame(height: 38)
        Rectangle().fill(Color(rgba: look.border)).frame(height: 1)
        if answer.rows.isEmpty {
            Text("Nenhum processo filho").font(.app(12)).foregroundStyle(Color(rgba: look.muted)).frame(height: 56)
        } else {
            ScrollView(.vertical) {
                VStack(spacing: 0) {
                    ForEach(answer.rows) { row in
                        ProcessRow(row: row, look: look) { stop(row.pid) }
                    }
                }
                .padding(.vertical, 4)
            }
            .frame(maxHeight: 320)
            .fixedSize(horizontal: false, vertical: true)
        }
    }

    private func load() async {
        let reply = await MagoStore.shared.act("ui.terminal.processes", ["id": id])
        guard reply.ok, let decoded = try? JSONDecoder().decode(Answer.self, from: Data(reply.text.utf8)) else { return }
        answer = decoded
    }

    private func stop(_ pid: Int) {
        Task {
            let reply = await MagoStore.shared.act("terminal_processes", ["name": id, "stop_pid": pid])
            if !reply.ok { ShellModel.shared.say(reply.text) }
            // the list again a moment later, once it went
            try? await Task.sleep(for: .milliseconds(500))
            await load()
        }
    }
}

/// A process: its name over its PID, its memory before the ×; the row lit under the pointer.
private struct ProcessRow: View {
    let row: TerminalProcesses.Row
    let look: TerminalProcesses.Look
    let stop: () -> Void
    @State private var hovering = false
    @State private var overStop = false

    var body: some View {
        HStack(spacing: 0) {
            VStack(alignment: .leading, spacing: 3) {
                Text(row.name).font(.app(12.5)).foregroundStyle(Color(rgba: look.text))
                Text("PID \(row.pid)").font(.code(10.5)).foregroundStyle(Color(rgba: look.muted))
            }
            .lineLimit(1)
            Spacer(minLength: 10)
            Text(row.memory).font(.code(11)).foregroundStyle(Color(rgba: look.text)).padding(.trailing, 8)
            Button(action: stop) {
                Image(systemName: "xmark").font(.system(size: 8, weight: .semibold))
                    .foregroundStyle(Color(rgba: overStop ? look.text : look.muted))
                    .frame(width: 22, height: 22)
                    .background(Color(rgba: look.hover).opacity(overStop ? 1 : 0), in: Circle())
                    .contentShape(Circle())
            }
            .buttonStyle(.plain)
            .onHover { overStop = $0 }
            .help("Parar \(row.name) (PID \(row.pid))")
            .accessibilityLabel("Parar \(row.name)")
        }
        .padding(.leading, 10).padding(.trailing, 5)
        .frame(height: 42)
        .background(Color(rgba: look.hover).opacity(hovering ? 1 : 0), in: RoundedRectangle(cornerRadius: 4))
        .padding(.horizontal, 4).padding(.vertical, 1)
        .onHover { hovering = $0 }
    }
}

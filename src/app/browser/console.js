(() => {
    // the page side of the agents' console: keeps the last console calls, script errors and unhandled
    // rejections in `window.__magoConsole`. It never breaks the page, and a second install does nothing.
    // The two limits (the uppercase names below) are filled in by console.rs.
    try {
        if (window.__magoConsoleInstalled) return;
        window.__magoConsoleInstalled = true;
        window.__magoConsole = [];

        // an Error has no own enumerable fields (JSON.stringify gives "{}"): its stack says more
        const show = (value) => {
            try {
                if (typeof value === 'string') return value;
                if (value instanceof Error) return value.stack || String(value);
                const json = JSON.stringify(value);
                return json === undefined ? String(value) : json;
            } catch (_) {
                return String(value);
            }
        };

        const add = (level, args) => {
            try {
                const text = Array.from(args).map(show).join(' ').slice(0, LONGEST);
                window.__magoConsole.push({level, text, time: Date.now()});
                if (window.__magoConsole.length > KEPT) {
                    window.__magoConsole.splice(0, window.__magoConsole.length - KEPT);
                }
            } catch (_) {
            }
        };

        for (const level of ['log', 'info', 'warn', 'error', 'debug']) {
            try {
                const original = console[level];
                console[level] = function (...args) {
                    add(level, args);
                    try {
                        return original.apply(this, args);
                    } catch (_) {
                    }
                };
            } catch (_) {
            }
        }

        window.addEventListener('error', (event) => {
            const where = event.filename ? '(' + event.filename + ':' + event.lineno + ')' : '';
            add('error', [event.error || event.message || 'Erro de script', where]);
        });

        window.addEventListener('unhandledrejection', (event) => {
            add('error', ['Promise rejeitada:', event.reason]);
        });
    } catch (_) {
    }
})();

// O comportamento do design system (design-system.html): a Luna, a ilha, o surgir com a rolagem, abas e cópia.
// Sem dependências; cada parte só age se o elemento existir na página.
(function () {
  var reduzido = window.matchMedia && matchMedia("(prefers-reduced-motion: reduce)").matches;

  // Movimento reduzido: a chama para de ondular (o <animate> do SVG não ouve o CSS)
  if (reduzido) document.querySelectorAll("svg").forEach(function (svg) { if (svg.pauseAnimations) svg.pauseAnimations(); });

  // ── A Luna: olha para o ponteiro (ou em volta, sozinha), pula ao passar o mouse e reage aos cliques (motion.rs poke)
  document.querySelectorAll(".luna-palco").forEach(function (palco) {
    var olhar = palco.querySelector(".luna-olhar"), cutucadas = 0, ultima = 0, ponteiro = false;
    function olha(x, y) { if (olhar) olhar.style.transform = "translate(" + (x * 6).toFixed(2) + "px," + (y * 4).toFixed(2) + "px)"; }
    function sozinha() {
      if (!ponteiro && !reduzido) olha(Math.random() * 1.6 - 0.8, Math.random() - 0.5);
      setTimeout(sozinha, 900 + Math.random() * 1800);
    }
    sozinha();
    if (!reduzido) window.addEventListener("pointermove", function (evento) {
      if (evento.pointerType !== "mouse") return;
      ponteiro = true;
      var caixa = palco.getBoundingClientRect();
      var dx = evento.clientX - (caixa.left + caixa.width / 2), dy = evento.clientY - (caixa.top + caixa.height * 0.65);
      var distancia = Math.max(Math.hypot(dx, dy), 1), alcance = Math.min(distancia / 260, 1);
      olha(dx / distancia * alcance, dy / distancia * alcance);
    });
    document.addEventListener("mouseleave", function () { ponteiro = false; olha(0, 0); });
    function so(classe, ms) {
      palco.classList.remove("pula", "cutucada", "tonta");
      void palco.offsetWidth;
      palco.classList.add(classe);
      clearTimeout(palco.fim);
      palco.fim = setTimeout(function () { palco.classList.remove(classe); }, ms);
    }
    palco.addEventListener("click", function () {
      var agora = Date.now();
      if (agora - ultima > 1500) cutucadas = 0;
      ultima = agora;
      cutucadas++;
      if (cutucadas >= 4) { cutucadas = 0; so("tonta", 2400); } else so("cutucada", 500);
    });
    palco.addEventListener("touchstart", function () { so("pula", 700); }, { passive: true });
  });

  // ── A ilha: abre e fecha sozinha (compacta 3,2 s, aberta 5,5 s); o ponteiro em cima a mantém aberta; clique ou Enter alternam
  document.querySelectorAll(".ilha[data-ciclo]").forEach(function (ilha) {
    var parada = false, relogio;
    function poe(aberta) {
      ilha.classList.toggle("aberta", aberta);
      ilha.setAttribute("aria-expanded", aberta);
    }
    function ciclo() {
      if (reduzido) return;
      relogio = setTimeout(function () {
        if (!parada) poe(!ilha.classList.contains("aberta"));
        ciclo();
      }, ilha.classList.contains("aberta") ? 5500 : 3200);
    }
    // movimento reduzido: fica aberta, sem ciclo
    if (reduzido) poe(true);
    ilha.addEventListener("mouseenter", function () { parada = true; poe(true); });
    ilha.addEventListener("mouseleave", function () { parada = false; clearTimeout(relogio); ciclo(); });
    ilha.addEventListener("click", function () { poe(!ilha.classList.contains("aberta")); });
    ilha.addEventListener("keydown", function (evento) {
      if (evento.key === "Enter" || evento.key === " ") { evento.preventDefault(); poe(!ilha.classList.contains("aberta")); }
    });
    ciclo();
  });

  // ── Surgir com a rolagem: só esconde quando dá para mostrar de novo
  var surgem = document.querySelectorAll(".surge");
  if (surgem.length && "IntersectionObserver" in window && !reduzido) {
    document.documentElement.classList.add("surgir");
    var observador = new IntersectionObserver(function (entradas) {
      entradas.forEach(function (entrada) {
        if (!entrada.isIntersecting) return;
        entrada.target.classList.add("visivel");
        observador.unobserve(entrada.target);
      });
    }, { rootMargin: "0px 0px -8% 0px", threshold: 0.12 });
    surgem.forEach(function (elemento) {
      // irmãos surgem um após o outro
      var irmaos = Array.prototype.filter.call(elemento.parentNode.children, function (outro) { return outro.classList.contains("surge"); });
      elemento.style.setProperty("--atraso", Math.min(irmaos.indexOf(elemento), 5) * 0.08 + "s");
      observador.observe(elemento);
    });
  }

  // ── Botões Copiar: copiam o texto do <pre> ao lado
  window.ligaCopiar = function (raiz) {
    raiz.querySelectorAll(".copiar").forEach(function (copiar) {
      copiar.onclick = function () {
        var bloco = copiar.previousElementSibling;
        function avisa(texto) {
          copiar.textContent = texto;
          setTimeout(function () { copiar.textContent = "Copiar"; }, 2000);
        }
        // sem permissão para a área de transferência, deixa o texto selecionado para o Ctrl+C
        navigator.clipboard.writeText(bloco.textContent).then(function () { avisa("Copiado"); }, function () {
          getSelection().selectAllChildren(bloco);
          avisa("Ctrl+C");
        });
      };
    });
  };
  window.ligaCopiar(document);

  // ── Abas (setas do teclado); as que têm data-sistema abrem no sistema de quem visita
  var agente = navigator.userAgent;
  var sistema = /Mac/.test(agente) && !/iPhone|iPad/.test(agente) ? "macos" : /Linux|X11/.test(agente) && !/Android/.test(agente) ? "linux" : "windows";
  document.querySelectorAll("[role=tablist]").forEach(function (lista) {
    var abas = Array.prototype.slice.call(lista.querySelectorAll("[role=tab]"));
    function escolhe(aba) {
      abas.forEach(function (outra) {
        var ativa = outra === aba;
        outra.setAttribute("aria-selected", ativa);
        outra.tabIndex = ativa ? 0 : -1;
        document.getElementById(outra.getAttribute("aria-controls")).hidden = !ativa;
      });
    }
    abas.forEach(function (aba, indice) {
      aba.onclick = function () { escolhe(aba); };
      aba.onkeydown = function (evento) {
        var passo = evento.key === "ArrowRight" ? 1 : evento.key === "ArrowLeft" ? -1 : 0;
        if (!passo) return;
        var proxima = abas[(indice + passo + abas.length) % abas.length];
        escolhe(proxima);
        proxima.focus();
      };
    });
    var doSistema = abas.filter(function (aba) { return aba.dataset.sistema === sistema; })[0];
    if (doSistema) escolhe(doSistema);
  });

  // ── Tema (só onde houver o botão, como no design system): alterna claro e escuro
  document.querySelectorAll("[data-alterna-tema]").forEach(function (botao) {
    botao.onclick = function () {
      var raiz = document.documentElement;
      var escuro = raiz.dataset.tema ? raiz.dataset.tema === "escuro" : !matchMedia("(prefers-color-scheme: light)").matches;
      raiz.dataset.tema = escuro ? "claro" : "escuro";
      botao.textContent = escuro ? "Ver no escuro" : "Ver no claro";
    };
  });
})();

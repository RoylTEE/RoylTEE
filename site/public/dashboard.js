/* Dashboard mockup: example data only. Built with textContent and DOM nodes. */
(function () {
  const SVG = 'http://www.w3.org/2000/svg';
  function el(tag, props, ...kids) {
    const n = document.createElement(tag);
    if (props) {
      for (const [k, v] of Object.entries(props)) {
        if (v == null || v === false) continue;
        if (k === 'className') n.className = v;
        else if (k === 'textContent') n.textContent = v;
        else if (k === 'style' && typeof v === 'object') Object.assign(n.style, v);
        else if (k === 'dataset') Object.assign(n.dataset, v);
        else if (k.startsWith('on') && typeof v === 'function') n[k.toLowerCase()] = v;
        else n.setAttribute(k, String(v));
      }
    }
    for (const kid of kids.flat()) if (kid != null && kid !== false) n.append(kid);
    return n;
  }
  function svg(tag, attrs, ...kids) {
    const n = document.createElementNS(SVG, tag);
    if (attrs) for (const [k, v] of Object.entries(attrs)) if (v != null) n.setAttribute(k, String(v));
    for (const kid of kids.flat()) if (kid != null && kid !== false) n.append(kid);
    return n;
  }
  function setMusdc(node, value) {
    node.replaceChildren(document.createTextNode(Number(value).toFixed(2) + ' '), el('small', { textContent: 'mUSDC' }));
  }
  function iconPlay() {
    return svg('svg', { width: '13', height: '13', viewBox: '0 0 24 24', fill: 'currentColor' }, svg('path', { d: 'M7 4.5v15l13-7.5z' }));
  }
  function iconPause() {
    return svg('svg', { width: '13', height: '13', viewBox: '0 0 24 24', fill: 'currentColor' },
      svg('rect', { x: '6', y: '4.5', width: '4', height: '15', rx: '1' }),
      svg('rect', { x: '14', y: '4.5', width: '4', height: '15', rx: '1' }));
  }
  function iconXl() {
    return svg('svg', { width: '12', height: '12', viewBox: '0 0 24 24', fill: 'none', stroke: 'currentColor', 'stroke-width': '2' },
      svg('path', { d: 'M14 4h6v6M20 4l-9 9M18 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h5' }));
  }

  const SONGS = [
    { id: 1, title: 'Harbour Lights', artist: 'Writer A, Writer B', prov: 'human', provL: 'human',
      isrc: 'US-RR1-26-00001', iswc: 'T-900.000.001-1', label: 'Demo Label (example)', pub: 'buyer-pub (from 2026-10-01)',
      splits: [['writer-a', 'publishing', '50%', 50, '#f08a5d', 'copyright'], ['writer-b', 'publishing', '50%', 50, '#f5a524', 'copyright'], ['label', 'master', '100%', 100, '#9b8cff', 'copyright']],
      tier: 'counterparty_confirmed', art: 'sunset',
      music: { bpm: 92, wave: 'triangle', bwave: 'sine', lead: [64, null, 67, 69, 71, null, 69, 67, 64, null, 62, 64, 67, null, null, null], bass: [40, null, null, null, 45, null, null, null, 43, null, null, null, 38, null, null, null], cut: 2400 } },
    { id: 2, title: 'Glass Circuit', artist: 'ai-co with Writer B', prov: 'ai_assisted', provL: 'AI-assisted',
      isrc: 'US-RR1-26-00002', iswc: 'T-900.000.002-2', label: 'Demo Label (example)', pub: 'writer-b',
      splits: [['writer-b', 'publishing', '60%', 60, '#f5a524', 'copyright'], ['ai-co', 'publishing', '40%', 40, '#2fd6c0', 'contract'], ['label', 'master', '100%', 100, '#9b8cff', 'copyright']],
      tier: 'payer_confirmed', art: 'circuit',
      music: { bpm: 118, wave: 'square', bwave: 'sawtooth', lead: [72, 76, 79, 76, 74, null, 71, 74, 72, 76, 79, 83, 81, null, 79, null], bass: [45, null, 45, null, 41, null, 41, null, 43, null, 43, null, 40, null, 47, null], cut: 1500 } },
    { id: 3, title: 'Null Horizon', artist: 'ai-co model output', prov: 'fully_ai', provL: 'fully AI',
      isrc: 'US-RR1-26-00003', iswc: 'none (no human author)', label: 'Demo Label (example)', pub: 'n/a',
      splits: [['ai-co', 'master', '60%', 60, '#9b8cff', 'contract'], ['label', 'master', '40%', 40, '#2fd6c0', 'contract'], ['claimant', 'master', '30%', 30, '#f2555a', 'disputed']],
      tier: 'self_asserted', art: 'horizon',
      music: { bpm: 76, wave: 'sine', bwave: 'triangle', lead: [69, 72, 76, 81, null, 79, 76, 72, 67, 71, 74, 79, null, 76, 74, 71], bass: [33, null, null, null, null, null, null, null, 31, null, null, null, null, null, null, null], cut: 3200, pad: true } }
  ];

  function art(kind) {
    const u = 'g' + Math.random().toString(36).slice(2, 7);
    if (kind === 'sunset') {
      const bands = [0, 1, 2, 3, 4, 5].map(i => svg('rect', { x: 0, y: 66 + i * 5, width: 100, height: 2 + i * 0.4, fill: '#2b1a4a', opacity: 0.35 + i * 0.1 }));
      return svg('svg', { viewBox: '0 0 100 100', width: '100%', height: '100%', preserveAspectRatio: 'xMidYMid slice' },
        svg('defs', {}, svg('linearGradient', { id: u + 'a', x1: '0', y1: '0', x2: '0', y2: '1' },
          svg('stop', { offset: '0', 'stop-color': '#2b1a4a' }), svg('stop', { offset: '.55', 'stop-color': '#d9534f' }), svg('stop', { offset: '1', 'stop-color': '#f7b267' }))),
        svg('rect', { width: 100, height: 100, fill: 'url(#' + u + 'a)' }),
        svg('circle', { cx: 50, cy: 62, r: 20, fill: '#ffd38a', opacity: '.95' }),
        ...bands,
        svg('text', { x: 8, y: 16, 'font-family': 'Helvetica,Arial', 'font-weight': 800, 'font-size': 9, fill: '#fff', opacity: '.9', 'letter-spacing': 1 }, document.createTextNode('HARBOUR')),
        svg('text', { x: 8, y: 26, 'font-family': 'Helvetica,Arial', 'font-weight': 300, 'font-size': 9, fill: '#fff', opacity: '.9', 'letter-spacing': 1 }, document.createTextNode('LIGHTS')));
    }
    if (kind === 'circuit') {
      const traces = Array.from({ length: 9 }, (_, i) => [
        svg('path', { d: 'M' + (8 + i * 10) + ' 100 V' + (40 + ((i * 37) % 40)) + ' H' + (14 + i * 10), stroke: i % 2 ? '#2fd6c0' : '#c084fc', 'stroke-width': '1.4', fill: 'none', opacity: '.8' }),
        svg('circle', { cx: 14 + i * 10, cy: 40 + ((i * 37) % 40), r: 1.8, fill: '#fff' })
      ]).flat();
      return svg('svg', { viewBox: '0 0 100 100', width: '100%', height: '100%' },
        svg('defs', {}, svg('linearGradient', { id: u + 'b', x1: '0', y1: '0', x2: '1', y2: '1' },
          svg('stop', { offset: '0', 'stop-color': '#0b2b3a' }), svg('stop', { offset: '1', 'stop-color': '#3b1361' }))),
        svg('rect', { width: 100, height: 100, fill: 'url(#' + u + 'b)' }),
        ...traces,
        svg('rect', { x: 30, y: 14, width: 40, height: 24, rx: 3, fill: 'none', stroke: '#fff', 'stroke-opacity': '.7' }),
        svg('text', { x: 50, y: 30, 'text-anchor': 'middle', 'font-family': 'Helvetica,Arial', 'font-weight': 800, 'font-size': 8, fill: '#fff', 'letter-spacing': 1 }, document.createTextNode('GLASS')));
    }
    const waves = Array.from({ length: 10 }, (_, i) => svg('path', { d: 'M0 ' + (60 + i * 4) + ' Q50 ' + (50 + i * 5) + ' 100 ' + (60 + i * 4), stroke: '#fff', 'stroke-opacity': 0.08 + i * 0.05, fill: 'none' }));
    return svg('svg', { viewBox: '0 0 100 100', width: '100%', height: '100%' },
      svg('defs', {}, svg('radialGradient', { id: u + 'c', cx: '.5', cy: '.85', r: '.9' },
        svg('stop', { offset: '0', 'stop-color': '#ff6ad5' }), svg('stop', { offset: '.35', 'stop-color': '#7a2cff' }), svg('stop', { offset: '1', 'stop-color': '#090418' }))),
      svg('rect', { width: 100, height: 100, fill: 'url(#' + u + 'c)' }),
      ...waves,
      svg('text', { x: 50, y: 22, 'text-anchor': 'middle', 'font-family': 'Courier New,monospace', 'font-weight': 700, 'font-size': 8, fill: '#fff', opacity: '.85', 'letter-spacing': 2 }, document.createTextNode('NULL//HORIZON')));
  }

  const songsEl = document.getElementById('songs');
  SONGS.forEach((s, i) => {
    const splits = s.splits.map(p => {
      const tags = [el('span', { className: 'who', textContent: p[0] }), el('span', { className: 'tag', textContent: p[1] })];
      if (p[5] === 'disputed') tags.push(el('span', { className: 'st no', textContent: 'disputed' }));
      else tags.push(el('span', { className: 'tag', style: { color: '#8f8b80' }, textContent: p[5] }));
      return el('div', { className: 'split' },
        el('div', { className: 'lb' }, ...tags),
        el('div', { className: 'pc', style: { color: p[5] === 'disputed' ? 'var(--red)' : 'inherit' }, textContent: p[2] }),
        el('div', { className: 'bar' }, el('b', { style: { width: Math.min(p[3], 100) + '%', background: p[4] } })));
    });
    const play = el('button', { className: 'play', dataset: { i: String(i) }, 'aria-label': 'Play ' + s.title });
    play.append(iconPlay());
    const c = el('div', { className: 'card song' },
      el('div', { className: 'top2' },
        el('div', { className: 'art' }, art(s.art)),
        el('div', { style: { minWidth: 0 } },
          el('div', { className: 'eyebrow', textContent: 'Work ' + s.id }),
          el('div', { className: 'title', textContent: s.title }),
          el('div', { className: 'artist', textContent: s.artist }),
          el('span', { className: 'prov ' + s.prov, textContent: s.provL }),
          el('span', { className: 'tier', style: { marginLeft: '4px' }, textContent: s.tier }))),
      el('div', { className: 'player' },
        play,
        el('div', {},
          el('canvas', { className: 'wave', dataset: { i: String(i) } }),
          el('div', { className: 'prog' }, el('span', { className: 'cur', textContent: '0:00' }), el('div', { className: 'track' }, el('div', { className: 'fill' })), el('span', { className: 'dur' })))),
      el('dl', { className: 'credits' },
        el('dt', { textContent: 'ISRC' }), el('dd', { textContent: s.isrc }),
        el('dt', { textContent: 'ISWC' }), el('dd', { textContent: s.iswc }),
        el('dt', { textContent: 'Label' }), el('dd', { textContent: s.label }),
        el('dt', { textContent: 'Publisher' }), el('dd', { textContent: s.pub }),
        el('dt', { textContent: 'Splits' }), el('dd', { className: 'splits', style: { whiteSpace: 'normal' } }, ...splits)));
    songsEl.appendChild(c);
  });

  document.getElementById('npArt').replaceChildren(art('sunset'));
  document.getElementById('dArt').replaceChildren(art('horizon'));

  function rng(seed) { return () => { seed = (seed * 16807) % 2147483647; return seed / 2147483647; }; }
  const WAVES = SONGS.map((s, i) => { const r = rng(31 + i * 977); const n = 90; const L = s.music.lead; return Array.from({ length: n }, (_, k) => { const step = Math.floor(k / n * 16); const on = L[step] != null ? 1 : 0.45; const env = 0.55 + 0.45 * Math.sin(k / n * Math.PI); return Math.max(0.12, Math.min(1, (0.35 + r() * 0.65) * on * env * 1.15)); }); });
  const progress = [0, 0, 0];
  function drawWave(i) {
    const cv = document.querySelector('canvas.wave[data-i="' + i + '"]'); const dpr = window.devicePixelRatio || 1;
    const w = cv.clientWidth, h = cv.clientHeight; if (cv.width !== w * dpr) { cv.width = w * dpr; cv.height = h * dpr; }
    const x = cv.getContext('2d'); x.setTransform(dpr, 0, 0, dpr, 0, 0); x.clearRect(0, 0, w, h);
    const b = WAVES[i], bw = w / b.length;
    b.forEach((v, k) => { const bh = v * h; x.fillStyle = (k / b.length) < progress[i] ? '#f5a524' : '#c9c6bd'; x.fillRect(k * bw + 0.5, (h - bh) / 2, Math.max(1, bw - 1.6), bh); });
  }
  let AC = null, master, analyser, splitter, anL, anR, cur = -1, timer = null, nextT = 0, stepIdx = 0, startT = 0;
  const mtof = m => 440 * Math.pow(2, (m - 69) / 12);
  function ensureAC() {
    if (AC) return; AC = new (window.AudioContext || window['webkitAudioContext'])();
    master = AC.createGain(); master.gain.value = 0.22;
    analyser = AC.createAnalyser(); analyser.fftSize = 256;
    splitter = AC.createChannelSplitter(2); anL = AC.createAnalyser(); anR = AC.createAnalyser(); anL.fftSize = anR.fftSize = 512;
    master.connect(analyser); analyser.connect(AC.destination); master.connect(splitter); splitter.connect(anL, 0); splitter.connect(anR, 1);
  }
  function voice(f, t, dur, type, vol, cut, panv) {
    const o = AC.createOscillator(), g = AC.createGain(), fl = AC.createBiquadFilter();
    o.type = type; o.frequency.value = f; fl.type = 'lowpass'; fl.frequency.value = cut;
    g.gain.setValueAtTime(0, t); g.gain.linearRampToValueAtTime(vol, t + 0.012); g.gain.exponentialRampToValueAtTime(0.0008, t + dur);
    let out = g; if (AC.createStereoPanner) { const p = AC.createStereoPanner(); p.pan.value = panv || 0; g.connect(p); out = p; }
    o.connect(fl); fl.connect(g); out.connect(master); o.start(t); o.stop(t + dur + 0.05);
  }
  function hat(t, vol) { const len = 0.04, buf = AC.createBuffer(1, AC.sampleRate * len, AC.sampleRate), d = buf.getChannelData(0); for (let k = 0; k < d.length; k++) d[k] = (Math.random() * 2 - 1) * (1 - k / d.length); const s = AC.createBufferSource(), f = AC.createBiquadFilter(), g = AC.createGain(); s.buffer = buf; f.type = 'highpass'; f.frequency.value = 7000; g.gain.value = vol; s.connect(f); f.connect(g); g.connect(master); s.start(t); }
  function kick(t) { const o = AC.createOscillator(), g = AC.createGain(); o.frequency.setValueAtTime(120, t); o.frequency.exponentialRampToValueAtTime(40, t + 0.15); g.gain.setValueAtTime(0.9, t); g.gain.exponentialRampToValueAtTime(0.001, t + 0.25); o.connect(g); g.connect(master); o.start(t); o.stop(t + 0.3); }
  function sched() {
    const m = SONGS[cur].music, sd = 60 / m.bpm / 2;
    while (nextT < AC.currentTime + 0.12) {
      const s = stepIdx % 16;
      if (m.lead[s] != null) voice(mtof(m.lead[s]), nextT, m.pad ? sd * 2.6 : sd * 1.6, m.wave, m.wave === 'square' ? 0.12 : 0.32, m.cut, (s % 2 ? 0.25 : -0.25));
      if (m.bass[s] != null) voice(mtof(m.bass[s]), nextT, m.pad ? sd * 8 : sd * 3, m.bwave, m.bwave === 'sawtooth' ? 0.14 : 0.4, m.pad ? 600 : 900, 0);
      if (m.pad && s % 8 === 0) { [0, 7, 12].forEach(iv => voice(mtof(57 + iv + (s ? -2 : 0)), nextT, sd * 8, 'sine', 0.07, 1800, iv ? 0.5 : -0.5)); }
      if (!m.pad) { if (s % 4 === 0) kick(nextT); if (s % 2 === 1) hat(nextT, cur === 1 ? 0.12 : 0.06); } else if (s % 4 === 2) hat(nextT, 0.03);
      nextT += sd; stepIdx++;
    }
  }
  function loopDur(i) { return 16 * 60 / SONGS[i].music.bpm / 2; }
  function fmt(t) { t = Math.max(0, t); return Math.floor(t / 60) + ':' + String(Math.floor(t % 60)).padStart(2, '0'); }
  const npTitle = document.getElementById('npTitle');
  const npMeta = document.getElementById('npMeta');
  const npCode = document.getElementById('npCode');
  function setNP(i) { const s = SONGS[i]; document.getElementById('npArt').replaceChildren(art(s.art)); npTitle.textContent = s.title; npMeta.textContent = s.artist; npCode.textContent = 'ISRC ' + s.isrc; }
  function stop() { if (timer) clearInterval(timer); timer = null; document.querySelectorAll('.play').forEach(b => { b.replaceChildren(iconPlay()); b.classList.remove('on'); }); cur = -1; }
  function play(i, fromFrac) {
    ensureAC(); AC.resume(); const was = cur; stop(); if (was === i && fromFrac == null) return;
    cur = i; setNP(i); const sd = 60 / SONGS[i].music.bpm / 2; stepIdx = Math.floor((fromFrac || progress[i]) * 16) % 16;
    nextT = AC.currentTime + 0.05; startT = nextT - stepIdx * sd; timer = setInterval(sched, 25); sched();
    const b = document.querySelector('.play[data-i="' + i + '"]'); b.replaceChildren(iconPause()); b.classList.add('on');
  }
  document.querySelectorAll('.play').forEach(b => { b.onclick = () => play(+b.dataset.i); });
  document.querySelectorAll('canvas.wave').forEach(cv => { cv.onclick = e => { const r = cv.getBoundingClientRect(); const f = (e.clientX - r.left) / r.width; progress[+cv.dataset.i] = f; play(+cv.dataset.i, f); }; });
  SONGS.forEach((_s, i) => { document.querySelectorAll('.song')[i].querySelector('.dur').textContent = fmt(loopDur(i)) + ' loop'; });
  progress[0] = 0.38;

  const vuL = document.getElementById('vuL'), vuR = document.getElementById('vuR'), spec = document.getElementById('spec');
  [vuL, vuR].forEach(b => { for (let k = 0; k < 28; k++) { const i = document.createElement('i'); if (k >= 19) i.className = k >= 25 ? 'r' : 'y'; b.appendChild(i); } });
  for (let k = 0; k < 32; k++) spec.appendChild(document.createElement('i'));
  function setVU(elNode, v) { const n = Math.round(v * 28); [...elNode.children].forEach((c, k) => c.classList.toggle('on', k < n)); }
  const fbuf = new Uint8Array(128), tb = new Float32Array(512);
  function rms(an) { an.getFloatTimeDomainData(tb); let s = 0; for (const v of tb) s += v * v; return Math.sqrt(s / tb.length); }
  let idleT = 0;
  function frame(ts) {
    if (cur >= 0 && AC) {
      const p = ((AC.currentTime - startT) % loopDur(cur)) / loopDur(cur); progress[cur] = p;
      const card = document.querySelectorAll('.song')[cur]; card.querySelector('.fill').style.width = (p * 100) + '%'; card.querySelector('.cur').textContent = fmt(p * loopDur(cur));
      setVU(vuL, Math.min(1, rms(anL) * 5.5)); setVU(vuR, Math.min(1, rms(anR) * 5.5));
      analyser.getByteFrequencyData(fbuf); [...spec.children].forEach((c, k) => { c.style.height = Math.max(2, fbuf[k * 2 + 2] / 255 * 40) + 'px'; });
    } else {
      idleT = ts / 1000; const a = 0.48 + 0.12 * Math.sin(idleT * 3.1) + 0.06 * Math.sin(idleT * 7.3), b = 0.45 + 0.12 * Math.sin(idleT * 2.7 + 1) + 0.06 * Math.sin(idleT * 6.1);
      setVU(vuL, a); setVU(vuR, b); [...spec.children].forEach((c, k) => { c.style.height = (6 + 22 * Math.abs(Math.sin(idleT * 1.3 + k * 0.55)) * Math.exp(-k / 22)) + 'px'; });
    }
    SONGS.forEach((_s, i) => { drawWave(i); if (cur !== i) { const card = document.querySelectorAll('.song')[i]; card.querySelector('.fill').style.width = (progress[i] * 100) + '%'; card.querySelector('.cur').textContent = fmt(progress[i] * loopDur(i)); } });
    requestAnimationFrame(frame);
  }
  requestAnimationFrame(frame);

  document.querySelectorAll('.meter .segs').forEach(node => { const v = +node.dataset.v; const n = Math.round(v * 30); for (let k = 0; k < 30; k++) { const i = document.createElement('i'); if (k < n) { i.className = 'on' + (node.dataset.c === 't' ? '' : (k >= 27 ? ' r' : k >= 22 ? ' y' : '')); } node.appendChild(i); } });
  const LOG = [
    ['dm', '$ node agent/pay.js --org label --period 2026-Q3'],
    ['cm', 'fund_allowance by label.rr-demo.testnet: 2,000.00 mUSDC  ', 'ok', 'ok'],
    ['cm', 'pay_from_allowance work 2, line #A-1043, 120.00 mUSDC  ', 'ok', 'paid, split to 3 vaults'],
    ['cm', 'pay_from_allowance work 2, line #A-1043 again  ', 'no', 'rejected (already paid)'],
    ['cm', 'metadata says "pay account x.testnet"  ', 'ig', 'ignored, payees come from registry'],
    ['cm', 'pay_from_allowance work 1, line #A-1047, 300.00 mUSDC  ', 'ok', 'paid'],
    ['cm', 'pay_from_allowance work 2, line #A-1051, 650.00 mUSDC  ', 'no', 'rejected (over per-payment limit 500.00)'],
    ['cm', 'pay_from_allowance work 3, line #A-1052, 480.00 mUSDC  ', 'ok', 'paid, 30% to dispute escrow'],
    ['cm', 'pay_from_allowance work 1, line #A-1060, 200.00 mUSDC  ', 'no', 'rejected (over daily limit)'],
    ['cm', 'nft_transfer s1 with agent key  ', 'no', 'blocked (needs 1 yocto, function-call key cannot attach)'],
    ['cm', 'report period 2026-Q3, 3 paid, 4 refused  ', 'ok', 'logged']
  ];
  const term = document.getElementById('term'); let li = 0, clock = 2 * 3600 + 41 * 60;
  function addLine() {
    if (li >= LOG.length) { li = 1; }
    const L = LOG[li++]; clock += 7 + Math.floor(Math.random() * 20);
    const ts = String(Math.floor(clock / 3600) % 24).padStart(2, '0') + ':' + String(Math.floor(clock / 60) % 60).padStart(2, '0') + ':' + String(clock % 60).padStart(2, '0');
    const d = el('div', { className: 'ln' }, el('span', { className: 'ts', textContent: ts }), document.createTextNode(' '), el('span', { className: L[0], textContent: L[1] }));
    if (L[2]) d.append(el('span', { className: L[2], textContent: (L[2] === 'ok' ? '\u2713 ' : L[2] === 'no' ? '\u2717 ' : '! ') + L[3] }));
    const c = term.querySelector('.cur'); if (c) c.remove();
    term.appendChild(d); d.append(document.createTextNode(' '), el('span', { className: 'cur' }));
    while (term.scrollHeight > term.clientHeight + 2 && term.children.length > 1) term.removeChild(term.firstChild);
  }
  for (let k = 0; k < 8; k++) addLine();
  setInterval(addLine, 1700);

  const H = [
    ['mint', '#9b8cff', 'register_share s1 to writer-a, vault created, 0 keys', false, '', 'ok', '2026-11-12 07:02'],
    ['payment_in', '#f5a524', 'payer, 2026-Q2, stmt 0x7c1e...9a02 ', true, '+200.00', 'ok', '2026-11-12 07:20'],
    ['claim', '#3ecf8e', 'writer-a claims while owner', false, '-200.00', 'ok', '2026-11-12 07:31'],
    ['transfer', '#2fd6c0', 'writer-a to buyer-pub, effective_from 2026-10-01', false, '', 'ok', '2026-11-13 02:12'],
    ['payment_in', '#f5a524', 'payer, 2026-Q3, stmt 0x41bd...0e77 ', true, '+120.00', 'ok', '2026-11-13 02:14'],
    ['claim', '#f2555a', 'writer-a claims after sale: not current owner', false, '0.00', 'no', '2026-11-13 02:15'],
    ['claim', '#3ecf8e', 'buyer-pub claims (balance travelled with token)', false, '-120.00', 'ok', '2026-11-13 02:16'],
    ['keys_check', '#5f5c54', 'view_access_key_list s1.royltee-demo.testnet: []', false, '', 'ok', '2026-11-13 02:16']
  ];
  const hist = document.getElementById('hist');
  hist.replaceChildren(...H.map(r => {
    const detail = el('td', { style: { color: '#2b2922' } }, document.createTextNode(r[2]));
    if (r[3]) detail.append(el('span', { className: 'sim', textContent: 'Simulated' }));
    const amtColor = r[4].startsWith('+') ? 'var(--amber2)' : r[4].startsWith('-') ? 'var(--green)' : 'var(--muted)';
    return el('tr', {},
      el('td', {}, el('span', { className: 'ev' }, el('span', { className: 'dot', style: { background: r[1] } }), document.createTextNode(r[0]))),
      detail,
      el('td', { className: 'mono', style: { textAlign: 'right', color: amtColor }, textContent: r[4] }),
      el('td', {}, el('span', { className: 'st ' + r[5], textContent: r[5] === 'ok' ? 'success' : 'failed' })),
      el('td', { className: 'mono muted', style: { whiteSpace: 'nowrap' }, textContent: r[6] }),
      el('td', {}, el('a', { className: 'xl', href: '#', title: 'Open on testnet explorer (example)' }, iconXl())));
  }));

  const stage = document.getElementById('stage'), nft = document.getElementById('nft');
  const $ = id => document.getElementById(id);
  function rect(node) { const s = stage.getBoundingClientRect(), r = node.getBoundingClientRect(); return { x: r.left - s.left, y: r.top - s.top, w: r.width, h: r.height, cx: r.left - s.left + r.width / 2, cy: r.top - s.top + r.height / 2 }; }
  let runId = 0, anims = [];
  const wait = (ms, id) => new Promise((res, rej) => setTimeout(() => id === runId ? res() : rej('stale'), ms));
  function place(node, x, y) { node.style.left = x + 'px'; node.style.top = y + 'px'; }
  function anim(node, kf, opt) { const a = node.animate(kf, Object.assign({ fill: 'forwards' }, opt)); anims.push(a); return a.finished; }
  function setStep(n) { const st = [...document.querySelectorAll('.step')]; st.forEach((s, k) => { if (k === n) { s.className = 'step on'; const bar = s.querySelector('i'); bar.style.transition = 'none'; bar.style.width = '0'; requestAnimationFrame(() => { bar.style.transition = 'width 2.2s linear'; bar.style.width = '100%'; }); } }); }
  function doneStep(n, bad) { const s = document.querySelectorAll('.step')[n]; s.className = 'step done' + (bad ? ' bad' : ''); s.querySelector('i').style.width = '0'; }
  function countTo(node, from, to, ms) { const t0 = performance.now(); return new Promise(r => { const f = t => { const p = Math.min(1, (t - t0) / ms); const e = 1 - Math.pow(1 - p, 3); setMusdc(node, from + (to - from) * e); p < 1 ? requestAnimationFrame(f) : r(); }; requestAnimationFrame(f); }); }
  function pingKeys() { const k = $('keys'); k.classList.remove('ping'); void k.offsetWidth; k.classList.add('ping'); }
  function reset() {
    anims.forEach(a => a.cancel()); anims = [];
    const A = rect($('laneA')); place(nft, A.x + (A.w - 184) / 2, 112); nft.style.transform = '';
    ['pPay', 'pSeller', 'pBuyer', 'pOut', 'vNo', 'vOk'].forEach(i => { $(i).style.opacity = 0; }); const ps0 = $('pSeller'); ps0.style.background = ''; ps0.style.color = ''; ps0.style.borderColor = '';
    setMusdc($('vbal'), 0); $('vown').textContent = 'writer-a'; $('nftOwner').textContent = 'owner: writer-a.rr-demo.testnet';
    $('laneA').classList.remove('glow'); $('laneB').classList.remove('glow');
    document.querySelectorAll('.step').forEach(s => { s.className = 'step'; s.querySelector('i').style.width = '0'; });
  }
  async function runSale() {
    const id = ++runId; reset(); pingKeys();
    try {
      $('laneA').classList.add('glow');
      await wait(900, id);
      setStep(0); const A = rect($('laneA')), B = rect($('laneB')); const dx = B.x - A.x;
      $('nftOwner').textContent = 'nft_transfer pending...';
      await anim(nft, [{ transform: 'translate(0,0) rotate(0) scale(1)' }, { transform: 'translate(' + (dx * 0.25) + 'px,-30px) rotate(-5deg) scale(1.07)', offset: 0.25 }, { transform: 'translate(' + (dx * 0.8) + 'px,-22px) rotate(3deg) scale(1.06)', offset: 0.72 }, { transform: 'translate(' + dx + 'px,0) rotate(0) scale(1)' }], { duration: 1900, easing: 'cubic-bezier(.45,.05,.25,1)' });
      anim(nft, [{ transform: 'translate(' + dx + 'px,0) scale(1)' }, { transform: 'translate(' + dx + 'px,6px) scale(.97,1.03)', offset: 0.35 }, { transform: 'translate(' + dx + 'px,-3px) scale(1.01)', offset: 0.7 }, { transform: 'translate(' + dx + 'px,0) scale(1)' }], { duration: 500, easing: 'ease-out' });
      $('laneA').classList.remove('glow'); $('laneB').classList.add('glow');
      $('nftOwner').textContent = 'owner: buyer-pub.rr-demo.testnet'; $('vown').textContent = 'buyer-pub'; pingKeys(); doneStep(0);
      await wait(800, id);
      setStep(1); const P = rect($('payer')), V = rect($('vault')), pp = $('pPay');
      place(pp, P.cx - 55, P.y + P.h + 4); pp.style.opacity = 1;
      await anim(pp, [{ transform: 'translateY(0) scale(.6)', opacity: 0 }, { transform: 'translateY(0) scale(1)', opacity: 1, offset: 0.15 }, { transform: 'translateY(' + (V.y - P.y - P.h - 14) + 'px) scale(1)', opacity: 1, offset: 0.75 }, { transform: 'translateY(' + (V.y - P.y - P.h - 2) + 'px) scale(.4)', opacity: 0 }], { duration: 1500, easing: 'cubic-bezier(.5,0,.75,0)' });
      anim($('vault'), [{ transform: 'translateX(-50%) scale(1)' }, { transform: 'translateX(-50%) scale(1.05)' }, { transform: 'translateX(-50%) scale(1)' }], { duration: 450, easing: 'cubic-bezier(.34,1.56,.64,1)', fill: 'none' });
      await countTo($('vbal'), 0, 120, 700); doneStep(1);
      await wait(500, id);
      setStep(2); const ps = $('pSeller'); const sx = A.x + A.w / 2 - 30, sy = V.cy - 14; place(ps, sx, sy);
      const tx = V.x - sx - 70;
      await anim(ps, [{ transform: 'translateX(0)', opacity: 0 }, { transform: 'translateX(0)', opacity: 1, offset: 0.1 }, { transform: 'translateX(' + tx + 'px)', opacity: 1 }], { duration: 850, easing: 'cubic-bezier(.5,0,.9,.5)' });
      anim($('vault'), [{ transform: 'translateX(-50%)' }, { transform: 'translateX(calc(-50% + 5px))' }, { transform: 'translateX(calc(-50% - 5px))' }, { transform: 'translateX(calc(-50% + 3px))' }, { transform: 'translateX(-50%)' }], { duration: 360, fill: 'none' });
      ps.style.background = '#fdecec'; ps.style.color = '#b8323a'; ps.style.borderColor = '#f0b9bc';
      const vn = $('vNo'); place(vn, A.x + 8, V.y - 36);
      anim(vn, [{ opacity: 0, transform: 'translateY(8px) scale(.9)' }, { opacity: 1, transform: 'translateY(0) scale(1)' }], { duration: 420, easing: 'cubic-bezier(.34,1.56,.64,1)' });
      await anim(ps, [{ transform: 'translateX(' + tx + 'px)' }, { transform: 'translateX(-10px)', offset: 0.7 }, { transform: 'translateX(0)', opacity: 0 }], { duration: 900, easing: 'cubic-bezier(.34,1.56,.64,1)' });
      doneStep(2, true); pingKeys();
      await wait(900, id);
      setStep(3); const pb = $('pBuyer'); const bx = B.x + B.w / 2 - 30, by = V.cy - 14; place(pb, bx, by);
      const tb2 = (V.x + V.w) - bx + 6;
      await anim(pb, [{ transform: 'translateX(0)', opacity: 0 }, { transform: 'translateX(0)', opacity: 1, offset: 0.1 }, { transform: 'translateX(' + tb2 + 'px)', opacity: 1 }], { duration: 800, easing: 'cubic-bezier(.5,0,.9,.5)' });
      anim(pb, [{ opacity: 1 }, { opacity: 0 }], { duration: 200 });
      const po = $('pOut'); place(po, V.x + V.w - 40, V.cy - 14);
      countTo($('vbal'), 120, 0, 900);
      await anim(po, [{ transform: 'translate(0,0) scale(.7)', opacity: 0 }, { transform: 'translate(10px,-8px) scale(1)', opacity: 1, offset: 0.2 }, { transform: 'translate(' + (B.cx - (V.x + V.w) - 10) + 'px,' + (B.y + B.h - 110 - V.cy) + 'px) scale(1)', opacity: 1, offset: 0.85 }, { transform: 'translate(' + (B.cx - (V.x + V.w) - 10) + 'px,' + (B.y + B.h - 110 - V.cy) + 'px) scale(.6)', opacity: 0 }], { duration: 1400, easing: 'cubic-bezier(.34,1.3,.64,1)' });
      const vo = $('vOk'); place(vo, B.x + B.w - 240, B.y + B.h - 84);
      anim(vo, [{ opacity: 0, transform: 'translateY(8px) scale(.9)' }, { opacity: 1, transform: 'translateY(0) scale(1)' }], { duration: 420, easing: 'cubic-bezier(.34,1.56,.64,1)' });
      doneStep(3); pingKeys();
    } catch (e) { /* stale run */ }
  }
  $('replay').onclick = runSale;
  window.addEventListener('load', () => setTimeout(runSale, 600));
  window.addEventListener('resize', () => { SONGS.forEach((_s, i) => drawWave(i)); });

  (function () {
    const shell = document.querySelector('.shell');
    document.getElementById('collapse').onclick = () => shell.classList.toggle('collapsed');
    document.getElementById('topReplay').onclick = () => { const r = document.getElementById('replay'); document.getElementById('shares').scrollIntoView({ behavior: 'smooth', block: 'center' }); r.click(); };
    const links = [...document.querySelectorAll('.side-scroll .ni[href^="#"]')].filter(a => a.getAttribute('href').length > 1 && document.querySelector(a.getAttribute('href')));
    const crumb = document.getElementById('crumbNow');
    function setOn(a) { document.querySelectorAll('.ni.on').forEach(x => x.classList.remove('on')); a.classList.add('on'); crumb.textContent = a.querySelector('.lbl').textContent; }
    links.forEach(a => a.addEventListener('click', e => { e.preventDefault(); document.querySelector(a.getAttribute('href')).scrollIntoView({ behavior: 'smooth', block: 'start' }); setOn(a); }));
    const order = ['#catalog', '#sharelist', '#songs', '#shares', '#agents', '#disputes', '#history'];
    addEventListener('scroll', () => { let curId = order[0]; for (const id of order) { const node = document.querySelector(id); if (node && node.getBoundingClientRect().top < 160) curId = id; } if (innerHeight + scrollY >= document.documentElement.scrollHeight - 4) curId = order[order.length - 1]; const a = links.find(l => l.getAttribute('href') === curId); if (a && !a.classList.contains('on')) setOn(a); }, { passive: true });
  })();

  (function () {
    const prov = [['human', '#23885a'], ['ai_assisted', '#6b5bd6'], ['fully_ai', '#c03fd0']];
    const lbl = { human: 'human', ai_assisted: 'AI-assisted', fully_ai: 'fully AI' };
    const cnt = prov.map(([k]) => SONGS.filter(x => x.prov === k).length); const tot = cnt.reduce((a, b) => a + b, 0);
    let off = 25;
    const ringKids = [svg('circle', { cx: 21, cy: 21, r: 15.9, fill: 'none', stroke: '#ecebe6', 'stroke-width': 5 })];
    prov.forEach(([, c], i) => { const pc = cnt[i] / tot * 100; ringKids.push(svg('circle', { cx: 21, cy: 21, r: 15.9, fill: 'none', stroke: c, 'stroke-width': 5, 'stroke-dasharray': (pc - 1.5) + ' ' + (100 - pc + 1.5), 'stroke-dashoffset': off })); off -= pc; });
    ringKids.push(svg('text', { x: 21, y: 23.5, 'text-anchor': 'middle', 'font-size': 8, 'font-weight': 700, fill: '#16150f', 'font-family': 'Inter Tight,Arial' }, document.createTextNode(String(tot))));
    $('ringSvg').replaceChildren(...ringKids);
    $('ringLeg').replaceChildren(...prov.map(([k, c], i) => el('li', {}, el('span', { className: 'dot', style: { background: c } }), document.createTextNode(lbl[k]), el('b', { textContent: String(cnt[i]) }))));
    const series = { 1: [0, 0, 40, 40, 120, 120, 200, 200, 320], 3: [0, 0, 0, 40, 40, 120, 200, 320, 320], 6: [0, 0, 0, 0, 0, 40, 120, 200, 320] };
    function draw(r) {
      const d = series[r], mx = 360, W = 300, H = 100; const pts = d.map((v, i) => [i / (d.length - 1) * W, H - 6 - (v / mx) * (H - 16)]);
      const line = pts.map((p, i) => (i ? 'L' : 'M') + p[0].toFixed(1) + ' ' + p[1].toFixed(1)).join(' ');
      const kids = [];
      for (let y = 1; y < 4; y++) kids.push(svg('line', { x1: 0, x2: W, y1: y * 25, y2: y * 25, stroke: '#efeee9', 'stroke-width': 1, 'vector-effect': 'non-scaling-stroke' }));
      kids.push(svg('path', { d: line + ' L' + W + ' ' + H + ' L0 ' + H + ' Z', fill: '#fdf1dc' }));
      kids.push(svg('path', { d: line, fill: 'none', stroke: '#f5a524', 'stroke-width': 2, 'vector-effect': 'non-scaling-stroke' }));
      $('lc').replaceChildren(...kids);
      $('lcCap').textContent = 'Simulated payer, last ' + r + ' month' + (r > 1 ? 's' : '') + ': ' + d[d.length - 1].toFixed(2) + ' mUSDC into s1';
      document.querySelectorAll('#rng button').forEach(b => b.classList.toggle('on', b.dataset.r == String(r)));
    }
    document.querySelectorAll('#rng button').forEach(b => { b.onclick = () => draw(+b.dataset.r); }); draw(3);
    const rows = []; let n = 0;
    SONGS.forEach(w => w.splits.forEach(sp => { n++; rows.push({ id: 's' + n, work: w, who: sp[0], right: sp[1], pct: sp[3], color: sp[4], basis: sp[5] }); }));
    const holder = r => r.id === 's1' ? 'buyer-pub' : r.who;
    function statusNode(r) {
      if (r.basis === 'disputed') return el('span', { className: 'st no', textContent: 'disputed, frozen' });
      if (r.id === 's7') return el('span', { className: 'st no', textContent: 'contested part frozen' });
      return el('span', { className: 'st ok', textContent: 'active' });
    }
    $('srows').replaceChildren(...rows.map(r => el('tr', { dataset: { id: r.id } },
      el('td', { className: 'mono' }, el('span', { className: 'dot', style: { background: r.color, marginRight: '6px' } }), document.createTextNode(r.id)),
      el('td', { textContent: r.work.title }),
      el('td', { textContent: r.right }),
      el('td', { className: 'who', textContent: holder(r) + '.rr-demo.testnet' }),
      el('td', {}, el('span', { className: 'tier', textContent: r.basis === 'disputed' ? 'contract' : r.basis })),
      el('td', { className: 'bps', style: { textAlign: 'right' }, textContent: (r.pct * 100).toLocaleString() }),
      el('td', {}, statusNode(r)),
      el('td', { className: 'chev', textContent: '\u203a' }))));
    function open(id) {
      const r = rows.find(x => x.id === id); if (!r) return;
      document.querySelectorAll('#srows tr').forEach(t => t.classList.toggle('sel', t.dataset.id === id));
      $('drT').textContent = r.id + ': ' + r.work.title + ', ' + r.who + ', ' + r.pct + '% ' + r.right;
      $('drS').textContent = 'ISRC ' + r.work.isrc + ', holder ' + holder(r) + '.rr-demo.testnet';
      $('drVault').textContent = r.id + '.royltee-demo.testnet';
      const chips = [['Right', r.right], ['Territory', 'World'], ['Basis', r.basis === 'disputed' ? 'contract' : r.basis], ['Points', (r.pct * 100) + ' bps'], ['Tier', r.work.tier], ['Provenance', lbl[r.work.prov]]];
      $('drChips').replaceChildren(...chips.map(c => el('span', { className: 'chip2' }, el('b', { textContent: c[0] }), document.createTextNode(c[1]))));
      const s1 = r.id === 's1';
      $('drKv').replaceChildren(
        el('div', {}, el('span', { className: 'eyebrow', textContent: 'Earned' }), el('b', { textContent: s1 ? '320.00' : '0.00' })),
        el('div', {}, el('span', { className: 'eyebrow', textContent: 'Paid out' }), el('b', { textContent: s1 ? '320.00' : '0.00' })),
        el('div', {}, el('span', { className: 'eyebrow', textContent: 'Claimable' }), el('b', { textContent: '0.00' })));
      let iss = [];
      if (s1) iss = [['payment_in', 'Q2 statement, simulated payer', '', '+200.00'], ['payment_in', 'Q3 statement, simulated payer', '', '+120.00'], ['claim', 'seller claim after sale rejected', { sev: 'lo', text: 'Low' }, '0.00']];
      if (r.id === 's7') iss = [['dispute #1', 'registered shares sum to 130%, contested 30% frozen', { sev: 'hi', text: 'High' }, 'held 60.00']];
      if (r.basis === 'disputed') iss = [['dispute #1', 'duplicate claim on the master', { sev: 'hi', text: 'High' }, 'frozen']];
      if (r.work.id === 2 && r.who === 'ai-co') iss = [['agent payment', 'repeated statement line rejected', { sev: 'md', text: 'Medium' }, '0.00']];
      if (!iss.length) iss = [['none', 'no payouts or issues in example data', '', '']];
      $('drIss').replaceChildren(...iss.map(i => el('tr', {},
        el('td', { className: 'mono', style: { fontSize: '11px' }, textContent: i[0] }),
        el('td', { textContent: i[1] }),
        el('td', {}, i[2] && typeof i[2] === 'object' ? el('span', { className: 'sev ' + i[2].sev, textContent: i[2].text }) : null),
        el('td', { className: 'mono', style: { textAlign: 'right' }, textContent: i[3] }))));
      const frozen = r.basis === 'disputed' || r.id === 's7';
      const acc = [[holder(r) + '.rr-demo.testnet', 'holder', frozen ? 'held' : 'yes', frozen ? 'no, frozen' : 'yes, 1 yocto'], ['registrar.rr-demo.testnet', 'registrar', 'no', 'only with a document hash'], ['pay-agent.label.rr-demo.testnet', 'agent allowance', 'no', 'no, pays works only']];
      $('drAcc').replaceChildren(...acc.map(a => el('tr', {},
        el('td', { className: 'mono', style: { fontSize: '11px' }, textContent: a[0] }),
        el('td', { textContent: a[1] }), el('td', { textContent: a[2] }), el('td', { textContent: a[3] }))));
      document.body.classList.add('dopen');
    }
    function close() { document.body.classList.remove('dopen'); document.querySelectorAll('#srows tr.sel').forEach(t => t.classList.remove('sel')); }
    $('srows').addEventListener('click', e => { const tr = e.target.closest('tr'); if (tr) open(tr.dataset.id); });
    $('drX').onclick = close; $('scrim').onclick = close; addEventListener('keydown', e => { if (e.key === 'Escape') close(); });
    window.__openShare = open;
    document.querySelectorAll('#tabs a').forEach(a => a.addEventListener('click', () => { document.querySelectorAll('#tabs a').forEach(x => x.classList.toggle('on', x === a)); }));
  })();
})();

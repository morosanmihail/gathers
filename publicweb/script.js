/* ═══════════════════════════════════════════════
   GatheRs public site — script.js
   ═══════════════════════════════════════════════ */

// ── Mobile nav toggle ──────────────────────────
const navToggle = document.getElementById('navToggle');
const navLinks  = document.querySelector('.nav-links');

navToggle.addEventListener('click', () => {
  navLinks.classList.toggle('open');
  navToggle.setAttribute('aria-expanded', navLinks.classList.contains('open'));
});

// Close mobile nav when a link is clicked
navLinks.querySelectorAll('a').forEach(a => {
  a.addEventListener('click', () => navLinks.classList.remove('open'));
});

// ── Tabs (Web UI section) ──────────────────────
const tabBtns   = document.querySelectorAll('.tab-btn');
const tabPanels = document.querySelectorAll('.tab-panel');

tabBtns.forEach(btn => {
  btn.addEventListener('click', () => {
    const target = btn.dataset.tab;

    tabBtns.forEach(b  => b.classList.remove('active'));
    tabPanels.forEach(p => p.classList.remove('active'));

    btn.classList.add('active');
    const panel = document.getElementById('tab-' + target);
    if (panel) panel.classList.add('active');
  });
});

// ── Install method tabs ────────────────────────
const installTabBtns   = document.querySelectorAll('.install-tab-btn');
const installTabPanels = document.querySelectorAll('.install-tab-panel');

installTabBtns.forEach(btn => {
  btn.addEventListener('click', () => {
    const target = btn.dataset.installTab;

    installTabBtns.forEach(b   => b.classList.remove('active'));
    installTabPanels.forEach(p => p.classList.remove('active'));

    btn.classList.add('active');
    const panel = document.getElementById('install-tab-' + target);
    if (panel) panel.classList.add('active');
  });
});

// ── Scroll-reveal via IntersectionObserver ────
const revealTargets = document.querySelectorAll('[data-animate], .feature-card, .game-card, .install-step, .api-group, .terminal-wrap, .collection-features, .collection-screenshot, .env-table-wrap');

const revealObserver = new IntersectionObserver((entries) => {
  entries.forEach((entry, i) => {
    if (entry.isIntersecting) {
      // Stagger siblings within the same parent
      const siblings = [...(entry.target.parentElement?.children ?? [])];
      const idx = siblings.indexOf(entry.target);
      const delay = idx * 80;

      setTimeout(() => {
        entry.target.classList.add('visible');
      }, delay);

      revealObserver.unobserve(entry.target);
    }
  });
}, { threshold: 0.12 });

revealTargets.forEach(el => revealObserver.observe(el));

// ── Active nav link highlighting ───────────────
const sections = document.querySelectorAll('section[id], header[id]');
const navAnchors = document.querySelectorAll('.nav-links a[href^="#"]');

const sectionObserver = new IntersectionObserver((entries) => {
  entries.forEach(entry => {
    if (entry.isIntersecting) {
      const id = entry.target.id;
      navAnchors.forEach(a => {
        a.classList.toggle('active-link', a.getAttribute('href') === '#' + id);
      });
    }
  });
}, { rootMargin: '-40% 0px -55% 0px' });

sections.forEach(s => sectionObserver.observe(s));

// Inject a tiny active-link style
const style = document.createElement('style');
style.textContent = '.nav-links a.active-link { color: var(--gold) !important; }';
document.head.appendChild(style);

// ── Navbar shrink on scroll ────────────────────
const navbar = document.querySelector('.navbar');
let lastY = 0;

window.addEventListener('scroll', () => {
  const y = window.scrollY;
  if (y > 80) {
    navbar.style.background = 'rgba(13,13,26,0.97)';
    navbar.style.borderBottomColor = 'rgba(200,168,75,0.25)';
  } else {
    navbar.style.background = 'rgba(13,13,26,0.88)';
    navbar.style.borderBottomColor = 'rgba(200,168,75,0.18)';
  }

  // Hide on scroll down, reveal on scroll up
  if (y > lastY && y > 200) {
    navbar.style.transform = 'translateY(-100%)';
  } else {
    navbar.style.transform = '';
  }
  lastY = y;
}, { passive: true });

navbar.style.transition = 'transform 0.3s ease, background 0.3s ease, border-color 0.3s ease';

// ── Smooth scroll for anchor links ────────────
document.querySelectorAll('a[href^="#"]').forEach(a => {
  a.addEventListener('click', e => {
    const target = document.querySelector(a.getAttribute('href'));
    if (!target) return;
    e.preventDefault();
    const navH = parseInt(getComputedStyle(document.documentElement).getPropertyValue('--nav-h')) || 64;
    const top  = target.getBoundingClientRect().top + window.scrollY - navH - 8;
    window.scrollTo({ top, behavior: 'smooth' });
  });
});

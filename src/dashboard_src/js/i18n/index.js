// Core i18n Engine for MCP Usage Dashboard
import { en } from './en.js';
import { vi } from './vi.js';

const dictionaries = { en, vi };
const listeners = new Set();

function detectInitialLanguage() {
  try {
    const saved = localStorage.getItem('agy_dashboard_lang');
    if (saved && (saved === 'en' || saved === 'vi')) return saved;
    const browserLang = (navigator.language || navigator.userLanguage || '').toLowerCase();
    return browserLang.startsWith('vi') ? 'vi' : 'en';
  } catch (e) {
    return 'en';
  }
}

let currentLanguage = detectInitialLanguage();

export function getLanguage() {
  return currentLanguage;
}

export function setLanguage(lang) {
  if (lang !== 'en' && lang !== 'vi') return;
  currentLanguage = lang;
  try {
    localStorage.setItem('agy_dashboard_lang', lang);
  } catch (e) {}
  document.documentElement.setAttribute('lang', lang);
  translateDom();
  listeners.forEach(fn => {
    try { fn(currentLanguage); } catch (err) { console.error('i18n listener error:', err); }
  });
}

export function t(key, params = {}) {
  const dict = dictionaries[currentLanguage] || dictionaries.en;
  let text = dict[key] ?? (typeof key === 'string' && key.includes('.') ? key.split('.').reduce((o, i) => o?.[i], dict) : undefined);
  if (text === undefined) {
    text = dictionaries.en[key] ?? (typeof key === 'string' && key.includes('.') ? key.split('.').reduce((o, i) => o?.[i], dictionaries.en) : undefined) ?? key;
  }
  if (typeof text === 'string' && Object.keys(params).length > 0) {
    Object.entries(params).forEach(([k, v]) => {
      text = text.replaceAll(`{${k}}`, v);
    });
  }
  return text;
}

export function onLanguageChange(callback) {
  listeners.add(callback);
  return () => listeners.delete(callback);
}

export function translateDom(root = document) {
  if (!root) return;
  root.querySelectorAll('[data-i18n]').forEach(el => {
    const key = el.getAttribute('data-i18n');
    if (key) el.textContent = t(key);
  });
  root.querySelectorAll('[data-i18n-placeholder]').forEach(el => {
    const key = el.getAttribute('data-i18n-placeholder');
    if (key) el.setAttribute('placeholder', t(key));
  });
  root.querySelectorAll('[data-i18n-title]').forEach(el => {
    const key = el.getAttribute('data-i18n-title');
    if (key) el.setAttribute('title', t(key));
  });
  root.querySelectorAll('[data-i18n-aria-label]').forEach(el => {
    const key = el.getAttribute('data-i18n-aria-label');
    if (key) el.setAttribute('aria-label', t(key));
  });
}

export function initI18n() {
  document.documentElement.setAttribute('lang', currentLanguage);
  translateDom();
}

import "./style.css";
import { initCore } from "./core";
import { el } from "./components";
import { hammingPage } from "./pages/hamming";
import { parityPage } from "./pages/parity";
import { parity2dPage } from "./pages/parity2d";
import { comparePage } from "./pages/compare";
import { crcPage } from "./pages/crc";
import type { Page } from "./pages/page";

const pages: Page[] = [parityPage, crcPage, parity2dPage, hammingPage, comparePage];

function renderNav(active: Page) {
  const nav = document.getElementById("nav")!;
  nav.replaceChildren(el("h1", { text: "ECC Lab" }));
  for (const group of new Set(pages.map((p) => p.group))) {
    nav.append(el("h2", { text: group }));
    for (const p of pages.filter((q) => q.group === group)) {
      const link = el("a", { class: p === active ? "active" : "", text: p.title });
      link.href = `#${p.id}`;
      nav.append(link);
    }
  }
}

function route() {
  const id = location.hash.slice(1);
  const page = pages.find((p) => p.id === id) ?? pages[0];
  renderNav(page);
  const main = document.getElementById("main")!;
  main.replaceChildren();
  page.mount(main);
}

async function start() {
  await initCore();
  window.addEventListener("hashchange", route);
  route();
}
start();

#!/usr/bin/env python3
"""Verify real Chromium geometry of mobile/desktop editor layout.

Runs the same Vite UI in a headless browser with mobile device metrics;
unlike source-only assertions, measures Header/Breadcrumb/Canvas rectangles.
"""

import json
import os
import shutil
import socket
import subprocess
import time
from pathlib import Path

from selenium import webdriver
from selenium.webdriver.chrome.options import Options
from selenium.webdriver.support.ui import WebDriverWait

UI_ROOT = Path(__file__).resolve().parents[1]
URL = "http://127.0.0.1:4173/"
METRICS = (
    (390, 844, True),
    (320, 568, True),
    (480, 900, True),
    (720, 450, False),
    (1440, 900, False),
)

JS_METRICS = r"""
const rect = (selector) => {
  const el = document.querySelector(selector);
  if (!el) return null;
  const r = el.getBoundingClientRect();
  return {x:r.x,y:r.y,width:r.width,height:r.height,bottom:r.bottom};
};
return {
  viewport: {width:window.innerWidth,height:window.innerHeight},
  document: {clientWidth:document.documentElement.clientWidth,
             scrollWidth:document.documentElement.scrollWidth},
  header:rect(".app-header"),
  breadcrumb:rect(".breadcrumbs"),
  workspace:rect(".workspace"),
  graph:rect(".graph-region"),
  canvas:rect(".canvas-surface"),
  inspector:rect(".inspector-region"),
  drawer:rect(".execution-drawer"),
  nodes:[...document.querySelectorAll(".react-flow__node")]
    .filter(node => node.querySelector(".algoram-node"))
    .map(node => ({
      width: parseFloat(getComputedStyle(node).width),
      height: parseFloat(getComputedStyle(node).height),
      label: node.querySelector(".node-title")?.textContent || "",
      ports: node.querySelectorAll(".algoram-handle").length,
      permanentTechChrome: Boolean(node.querySelector(
        ".node-grip,.node-badge,.node-subtitle,.node-port-grid"))
    })),
};
"""


def server_ready():
    try:
        with socket.create_connection(("127.0.0.1", 4173), timeout=0.3):
            return True
    except OSError:
        return False


def assert_geometry(m, width, height, execution_open=False):
    header = m["header"]
    breadcrumb = m["breadcrumb"]
    canvas = m["canvas"]
    workspace = m["workspace"]
    assert all((header, breadcrumb, canvas, workspace)), m
    assert m["document"]["scrollWidth"] <= m["document"]["clientWidth"] + 1, m
    assert header["height"] <= 105, f"Header stretched: {m}"
    assert breadcrumb["height"] <= 50, f"Breadcrumb stretched: {m}"
    assert abs(workspace["y"] - breadcrumb["bottom"]) <= 2, (
        f"Whitespace between nav and workspace: {m}"
    )
    assert canvas["height"] >= 170, f"Canvas collapsed: {m}"
    # W1: Check browser-computed intrinsic geometry, not only TS estimates.
    nodes = m["nodes"]
    assert len(nodes) >= 2, f"Canvas nodes missing: {m}"
    assert all(72 <= node["width"] < 244 for node in nodes), nodes
    assert all(node["height"] >= 56 for node in nodes), nodes
    assert any(node["label"] == "Py→C ×2" for node in nodes), nodes
    assert not any(node["permanentTechChrome"] for node in nodes), nodes
    # A substantial working area must be visible before the first scroll.
    visible_canvas = max(0, min(canvas["bottom"], m["viewport"]["height"]) -
                         max(canvas["y"], 0))
    assert visible_canvas >= (170 if height >= 568 else 120), (
        f"Canvas starts below the fold: {m}; visible={visible_canvas}"
    )
    if execution_open:
        assert m["drawer"] is not None, m
    print(json.dumps({"width": width, "height": height,
                      "execution_open": execution_open,
                      "header_height": round(header["height"], 2),
                      "breadcrumb_height": round(breadcrumb["height"], 2),
                      "canvas_y": round(canvas["y"], 2),
                      "canvas_height": round(canvas["height"], 2),
                      "visible_canvas": round(visible_canvas, 2),
                      "scroll_width": m["document"]["scrollWidth"],
                      "client_width": m["document"]["clientWidth"]},
                     ensure_ascii=False), flush=True)


def main():
    process = subprocess.Popen(
        ["npm", "run", "dev", "--", "--host", "127.0.0.1", "--port", "4173",
         "--strictPort"],
        cwd=UI_ROOT,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.STDOUT,
    )
    browser = None
    try:
        for _ in range(100):
            if process.poll() is not None:
                raise RuntimeError("Vite exited before serving")
            if server_ready():
                break
            time.sleep(0.2)
        else:
            raise RuntimeError("Vite did not start in 20 seconds")

        options = Options()
        executable = shutil.which("google-chrome") or shutil.which("chromium")
        if executable:
            options.binary_location = executable
        options.add_argument("--headless=new")
        options.add_argument("--no-sandbox")
        options.add_argument("--disable-dev-shm-usage")
        options.add_argument("--disable-gpu")
        browser = webdriver.Chrome(options=options)
        wait = WebDriverWait(browser, 20)
        for width, height, mobile in METRICS:
            browser.execute_cdp_cmd("Emulation.setDeviceMetricsOverride", {
                "width": width, "height": height,
                "deviceScaleFactor": 3 if mobile else 1,
                "mobile": mobile,
            })
            browser.get(URL)
            wait.until(lambda d: d.execute_script(
                "return Boolean(document.querySelector('.canvas-surface .react-flow'))"))
            # Allow font/layout and Graph presentation effects to settle.
            wait.until(lambda d: d.execute_script(
                "return document.querySelectorAll('.algoram-node').length >= 2"))
            time.sleep(0.3)
            m = browser.execute_script(JS_METRICS)
            assert_geometry(m, width, height)
            # W2a real DOM interaction: temporary global search never consumes Canvas.
            if width in (390, 1440):
                browser.find_element(
                    "css selector", ".canvas-toolbar-actions button[aria-label='＋ 能力を探す']"
                ).click()
                wait.until(lambda d: d.execute_script(
                    "return document.querySelectorAll('#block-palette .palette-item').length >= 2"
                ))
                assert_geometry(browser.execute_script(JS_METRICS), width, height)
                browser.find_element("css selector", "#block-palette .palette-close").click()
            if width == 1440:
                browser.find_element("css selector", ".react-flow__node").click()
                wait.until(lambda d: d.execute_script(
                    "return Boolean(document.querySelector('.node-local-add'))"
                ))
                browser.find_element("css selector", ".node-local-add").click()
                wait.until(lambda d: d.execute_script(
                    "return Boolean(document.querySelector('#block-palette [data-connection-discovery]'))"
                ))
                route_labels = browser.execute_script(
                    "return Array.from(document.querySelectorAll('#block-palette [data-connection-discovery]'),"
                    " el => el.getAttribute('data-connection-discovery'))"
                )
                assert route_labels and all(label != "verified" for label in route_labels), route_labels
                assert "needs-mapper-or-route" in route_labels, route_labels
                browser.find_element("css selector", "#block-palette .palette-close").click()
            if width in (390, 320, 1440):
                browser.find_element("css selector", ".execution-entry-action").click()
                wait.until(lambda d: d.execute_script(
                    "return document.querySelector('.app-shell').classList.contains('execution-open')"))
                assert_geometry(browser.execute_script(JS_METRICS), width, height,
                                execution_open=True)
        print("PASS: native Chrome responsive header/breadcrumb/canvas geometry", flush=True)
    finally:
        if browser is not None:
            browser.quit()
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()


if __name__ == "__main__":
    main()

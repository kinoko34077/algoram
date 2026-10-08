import json, os, time, traceback
from selenium import webdriver
from selenium.webdriver.chrome.options import Options
from selenium.webdriver.common.by import By
from selenium.webdriver.common.keys import Keys
from selenium.webdriver.support.ui import WebDriverWait

opts=Options()
for arg in ("--headless=new","--no-sandbox","--disable-dev-shm-usage"):
    opts.add_argument(arg)
opts.add_experimental_option("prefs", {"download.default_directory":"/tmp/u7-downloads","download.prompt_for_download":False})
opts.set_capability("goog:loggingPrefs", {"browser":"ALL"})
os.makedirs("/tmp/u7-downloads",exist_ok=True)
d=webdriver.Chrome(options=opts)
W=WebDriverWait(d,35)
results=[]

def until(fn, note):
    try: return W.until(fn)
    except Exception:
        print("FAILED_AT:",note,flush=True)
        print("BODY_SAMPLE:",d.find_element(By.TAG_NAME,"body").text[:4000],flush=True)
        print("URL:",d.current_url,flush=True)
        raise

def item(selector):
    return d.find_element(By.CSS_SELECTOR,selector)

def visible(selector):
    return [e for e in d.find_elements(By.CSS_SELECTOR,selector) if e.is_displayed()]

def click_text(selector,needle):
    for el in visible(selector):
        if needle in el.text and el.is_enabled():
            d.execute_script("arguments[0].scrollIntoView({block:'center'})",el)
            el.click()
            return el
    raise AssertionError("button not found: "+needle+" within "+selector)

def record(step,**more):
    result={"step":step,"status":"PASS",**more}
    print(json.dumps(result,ensure_ascii=False),flush=True)
    results.append(result)

try:
    d.set_window_size(1440,900)
    d.get("http://127.0.0.1:5173/")
    until(lambda x:visible(".app-shell"),"initial editor")
    assert d.execute_script("return document.documentElement.lang")=="ja"
    assert visible(".graph-canvas") and visible(".canvas-surface")
    assert visible(".canvas-first-use")
    record("first_use_and_canvas",guide_steps=len(visible(".canvas-first-use li")))
    click_text(".canvas-first-use button","×") if False else visible(".canvas-first-use button")[0].click()
    assert not visible(".canvas-first-use")
    record("guide_dismissed")

    item('.visually-hidden-file-input').send_keys('/tmp/u7-fixture.algoram.json')
    until(lambda x:"U7 操作検証" in item(".title-block h1").text,"import editable graph")
    until(lambda x:len(visible(".react-flow__node"))==2,"initial nodes")
    record("file_import",node_count=len(visible(".react-flow__node")))

    until(lambda x:visible(".palette-actions button"),"palette")
    click_text(".palette-actions button","ブロックを追加")
    until(lambda x:len(visible(".react-flow__node"))==3,"added node")
    record("add_block",nodes=3)
    click_text(".desktop-history-actions button","元に戻す")
    until(lambda x:len(visible(".react-flow__node"))==2,"undo")
    click_text(".desktop-history-actions button","やり直す")
    until(lambda x:len(visible(".react-flow__node"))==3,"redo")
    record("undo_redo")

    provider=until(lambda x:next((n for n in visible(".react-flow__node") if "U7 provider" in n.text),None),"provider rendered")
    d.execute_script("arguments[0].scrollIntoView({block:'center'})",provider)
    provider.click()
    until(lambda x:visible(".block-inspector h2") and "U7 provider" in item(".block-inspector h2").text,"selected Inspector")
    until(lambda x:visible(".draft-link-form button"),"connection editor")
    click_text(".draft-link-form button","接続を作成")
    until(lambda x:len(visible(".react-flow__edge"))>=1,"connection created")
    record("connect_and_inspect",edges=len(visible(".react-flow__edge")))

    click_text(".file-menu > summary","ファイル")
    click_text(".file-menu-popover button","ローカル保存")
    until(lambda x:visible(".file-operation-status"),"save status")
    record("local_save",message=item(".file-operation-status").text[:120])

    click_text(".file-menu > summary","ファイル")
    click_text(".file-menu-popover button","書き出し")
    until(lambda x:any(n.endswith(".algoram.json") for n in os.listdir("/tmp/u7-downloads")),"completed export download")
    record("export",files=os.listdir("/tmp/u7-downloads"))

    d.refresh()
    until(lambda x:visible(".app-shell") and "U7 操作検証" in item(".title-block h1").text,"local restore")
    until(lambda x:len(visible(".react-flow__node"))==3,"restored nodes")
    assert len(visible(".react-flow__edge"))>=1
    record("reopen_reuse",nodes=len(visible(".react-flow__node")),edges=len(visible(".react-flow__edge")))

    click_text(".execution-entry-action","実行")
    until(lambda x:visible(".execution-drawer"),"execution drawer")
    assert "ローカルブリッジ" in item(".execution-drawer-heading").text
    assert not visible(".execution-preview")
    record("readiness_before_plan",copy=item(".execution-drawer-heading").text[:120])

    click_text(".bridge-settings > summary","ブリッジ")
    token=item('.bridge-settings input[type=password]')
    token.send_keys("u7-local-test-token")
    click_text(".execution-drawer-actions button","実行計画")
    until(lambda x:visible(".execution-preview") and visible(".execution-grant input[type=checkbox]"),"actual plan+grant")
    grant=visible(".execution-grant input[type=checkbox]")[0]
    assert not grant.is_selected()
    runbuttons=[e for e in visible(".execution-drawer-actions button") if e.text.strip()=="実行"]
    assert runbuttons and not runbuttons[0].is_enabled()
    grant.click()
    assert runbuttons[0].is_enabled()
    record("real_bridge_plan_and_explicit_grant",requirements=len(visible(".execution-grant")))

    click_text(".execution-drawer-actions button","実行")
    until(lambda x:visible(".execution-state.failed"),"failed run trace")
    until(lambda x:visible(".recovery-panel"),"recovery options")
    record("real_bridge_failed_run_and_recovery_discovery")

    until(lambda x:any("impl:u7-good" in e.text for e in visible(".recovery-choice")),"alternative candidate")
    good=next(e for e in visible(".recovery-choice") if "impl:u7-good" in e.text)
    good.find_element(By.CSS_SELECTOR,'input[type=radio]').click()
    click_text(".recovery-panel button","適用して再計画")
    until(lambda x:visible(".execution-state.ready") and any("impl:u7-good" in e.text for e in visible(".execution-grant")),"recovery replanned")
    goodGrant=visible(".execution-grant input[type=checkbox]")[0]
    assert not goodGrant.is_selected()
    goodGrant.click()
    click_text(".execution-drawer-actions button","実行")
    until(lambda x:visible(".execution-state.succeeded"),"recovery run succeeds")
    record("real_bridge_recovery_rerun",provider="impl:u7-good")

    for width,height in ((1440,900),(720,450),(480,900),(320,450)):
        d.set_window_size(width,height)
        time.sleep(.5)
        scroll,client=d.execute_script("return [document.documentElement.scrollWidth,document.documentElement.clientWidth]")
        assert scroll<=client+1,(width,scroll,client)
        record("responsive_reflow",width=width,height=height,client_width=client,scroll_width=scroll)

    serious=[e for e in d.get_log("browser") if e["level"]=="SEVERE" and "favicon.ico" not in e["message"]]
    assert not serious,serious
    record("browser_console",severe=0)

    d.set_window_size(1440,900)
    d.get("https://kinoko34077.github.io/algoram/")
    until(lambda x:visible(".app-shell"),"published Pages shell")
    assert d.execute_script("return document.documentElement.lang")=="ja"
    assert visible(".graph-canvas")
    record("live_pages_shell",url=d.current_url)

    print(json.dumps({"result":"PASS","checks":len(results),"note":"Local Vite→real guarded loopback bridge, not live Pages→loopback"},ensure_ascii=False),flush=True)
except BaseException:
    traceback.print_exc()
    try:
        print("DIAGNOSTIC_BODY:",d.find_element(By.TAG_NAME,"body").text[:6000],flush=True)
        print("BROWSER_LOG:",json.dumps(d.get_log("browser")[-12:]),flush=True)
    except Exception: pass
    raise
finally:
    d.quit()

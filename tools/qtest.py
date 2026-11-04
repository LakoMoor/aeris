"""Drive QEMU through its monitor: boot an image, send input, take screenshots.

usage: python tools/qtest.py IMG OUT_PREFIX [uefi] -- step step ...
steps: wait:SEC  shot:NAME  key:QEMUKEY  type:TEXT  move:DX,DY  click  raw:MONITOR_CMD
"""
import os
import socket
import subprocess
import sys
import time

img, prefix = sys.argv[1], sys.argv[2]
rest = sys.argv[3:]
uefi = "uefi" in rest
steps = rest[rest.index("--") + 1:] if "--" in rest else []
qemu = r"C:\Program Files\qemu\qemu-system-x86_64.exe"
port = 4455
args = [qemu, "-m", "1024", "-vga", "std", "-display", "none", "-monitor", f"tcp:127.0.0.1:{port},server,nowait",
        "-drive", f"format=raw,file={img},if=ide,index=0", "-serial", "file:" + prefix + "serial.log"]
data = os.environ.get("AERIS_DATA")
if data:
    args += ["-drive", f"format=raw,file={data},if=ide,index=1"]
if uefi:
    args += ["-drive", r"if=pflash,format=raw,readonly=on,file=C:\Program Files\qemu\share\edk2-x86_64-code.fd"]
p = subprocess.Popen(args)
time.sleep(1.5)
s = socket.create_connection(("127.0.0.1", port))
s.settimeout(2)


def rd():
    try:
        return s.recv(65536).decode(errors="ignore")
    except Exception:
        return ""


rd()


def cmd(c):
    s.sendall((c + "\n").encode())
    time.sleep(0.15)
    return rd()


def shot(name):
    out = (prefix + name + ".ppm").replace(chr(92), "/")
    cmd(f"screendump {out}")
    time.sleep(0.5)
    try:
        from PIL import Image
        Image.open(out).save(prefix + name + ".png")
        os.remove(out)
    except Exception as e:
        print("png convert failed", e)


KEYMAP = {" ": "spc", "-": "minus", ".": "dot", "/": "slash", chr(92): "backslash", ":": "shift-semicolon",
          ">": "shift-dot", "|": "shift-backslash", "$": "shift-4", '"': "shift-apostrophe", "'": "apostrophe",
          "_": "shift-minus", "~": "shift-grave_accent", "=": "equal", ",": "comma", ";": "semicolon"}

try:
    for st in steps:
        k, _, v = st.partition(":")
        if k == "wait":
            time.sleep(float(v))
        elif k == "shot":
            shot(v)
        elif k == "key":
            cmd(f"sendkey {v}")
            time.sleep(0.1)
        elif k == "type":
            for ch in v:
                key = KEYMAP.get(ch, ("shift-" + ch.lower()) if ch.isupper() else ch)
                cmd(f"sendkey {key}")
                time.sleep(0.05)
        elif k == "move":
            dx, dy = (int(a) for a in v.split(","))
            while dx or dy:
                sx = max(-100, min(100, dx)); sy = max(-100, min(100, dy))
                cmd(f"mouse_move {sx} {sy}")
                dx -= sx; dy -= sy
                time.sleep(0.04)
            time.sleep(0.1)
        elif k == "click":
            cmd("mouse_button 1")
            time.sleep(0.12)
            cmd("mouse_button 0")
            time.sleep(0.15)
        elif k == "raw":
            print(cmd(v))
finally:
    cmd("quit")
    try:
        p.wait(timeout=10)
    except Exception:
        p.kill()

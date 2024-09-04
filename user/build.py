# 该脚本编译所有src/bin下的应用程序，并将其链接到不同的地址（不同的BASE_ADDRESS）
import os
import sys

base_address = 0x80400000
step = 0x20000
linker = 'src/linker.ld'

app_id = 0
apps = os.listdir('src/bin')
apps.sort()
for app in apps:
    app = app[:app.find('.')]
    lines = []
    lines_before = []
    with open(linker, 'r') as f:
        for line in f.readlines():
            lines_before.append(line)
            line = line.replace(hex(base_address), hex(base_address+step*app_id))
            lines.append(line)
    with open(linker, 'w+') as f:
        f.writelines(lines)
    try:
        exit_code = os.system('cargo build --bin %s --release' % app)
        if exit_code != 0:
            print('[build.py] application %s build failed' % app)
            sys.exit(1)
        print('[build.py] application %s start with address %s' %(app, hex(base_address+step*app_id)))
    finally:
        with open(linker, 'w+') as f:
            f.writelines(lines_before)
    app_id = app_id + 1

"""Draws the README figures, one before/after SVG per lint. Run from this directory: python3 figures.py"""
import math

STYLE = '''  <style>
    text { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif; font-size: 13px; fill: #1f2328; }
    .mono { font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; font-size: 12px; }
    .small { font-size: 12px; }
    .muted { fill: #656d76; }
    .bold { font-weight: 600; }
    .canvas { fill: #ffffff; stroke: #d0d7de; }
    .panel { fill: #f6f8fa; }
    .mod { fill: #ffffff; stroke: #8c959f; stroke-width: 1.2; }
    .box { fill: none; stroke: #afb8c1; stroke-width: 1.2; }
    .private { fill: #ffffff; stroke: #8c959f; stroke-width: 1.2; stroke-dasharray: 4 3; }
    .tree { stroke: #afb8c1; stroke-width: 1.5; fill: none; }
    .edge { stroke: #57606a; stroke-width: 1.5; fill: none; }
    .thin { stroke: #8c959f; stroke-width: 1.2; fill: none; }
    .bad { stroke: #cf222e; stroke-width: 1.8; fill: none; }
    .good { stroke: #1a7f37; stroke-width: 2; fill: none; }
    .badtext { fill: #cf222e; }
    .goodtext { fill: #1a7f37; }
    .badbox { fill: #ffebe9; stroke: #cf222e; stroke-width: 1.2; }
    .goodbox { fill: #dafbe1; stroke: #1a7f37; stroke-width: 1.2; }
    .tip-n { fill: #57606a; } .tip-t { fill: #8c959f; } .tip-b { fill: #cf222e; } .tip-g { fill: #1a7f37; }
    @media (prefers-color-scheme: dark) {
      text { fill: #e6edf3; }
      .muted { fill: #8d96a0; }
      .canvas { fill: #0d1117; stroke: #30363d; }
      .panel { fill: #161b22; }
      .mod, .private { fill: #0d1117; stroke: #6e7681; }
      .box, .tree { stroke: #484f58; }
      .edge { stroke: #8d96a0; }
      .thin { stroke: #6e7681; }
      .bad { stroke: #f85149; }
      .good { stroke: #3fb950; }
      .badtext { fill: #f85149; }
      .goodtext { fill: #3fb950; }
      .badbox { fill: #3c1618; stroke: #f85149; }
      .goodbox { fill: #12261e; stroke: #3fb950; }
      .tip-n { fill: #8d96a0; } .tip-t { fill: #6e7681; } .tip-b { fill: #f85149; } .tip-g { fill: #3fb950; }
    }
  </style>
  <defs>
''' + ''.join(
    f'    <marker id="{k}" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto"><path d="M0,0 L10,5 L0,10 z" class="tip-{k}"/></marker>\n'
    for k in 'ntbg') + '  </defs>\n'

HEIGHT = 320
FRAME = f'''  <rect class="canvas" x="0.5" y="0.5" width="799" height="{HEIGHT-1}" rx="8"/>
  <rect class="panel" x="10" y="10" width="380" height="{HEIGHT-20}" rx="6"/>
  <rect class="panel" x="410" y="10" width="380" height="{HEIGHT-20}" rx="6"/>
  <rect class="badbox" x="22" y="20" width="64" height="22" rx="11"/>
  <text class="bold small badtext" x="54" y="35" text-anchor="middle">Before</text>
  <rect class="goodbox" x="422" y="20" width="56" height="22" rx="11"/>
  <text class="bold small goodtext" x="450" y="35" text-anchor="middle">After</text>
'''

TEXT_CLASS = {'badbox': ' badtext', 'goodbox': ' goodtext'}


class Panel:
    """One side of a figure; coordinates are local to the left panel and shifted by `dx`."""

    def __init__(self, dx):
        self.dx = dx
        self.back = []
        self.front = []

    def node(self, x, y, label='', cls='mod', w=None, h=26):
        if w is None:
            w = len(label) * 7.3 + 20
        n = dict(x=x + self.dx, y=y, w=w, h=h)
        self.front.append(f'  <rect class="{cls}" x="{n["x"]-w/2:.1f}" y="{y-h/2:.1f}" width="{w:.1f}" height="{h}" rx="5"/>\n')
        if label:
            self.front.append(f'  <text class="mono{TEXT_CLASS.get(cls, "")}" x="{n["x"]:.1f}" y="{y+4}" text-anchor="middle">{label}</text>\n')
        return n

    def box(self, x, y, w, h, label):
        self.back.append(f'  <rect class="box" x="{x+self.dx}" y="{y}" width="{w}" height="{h}" rx="6"/>\n')
        self.back.append(f'  <text class="mono muted" x="{x+self.dx+10}" y="{y+18}">{label}</text>\n')

    def edge(self, a, b, kind='edge', via=None):
        """`kind`: edge, thin, bad, good (arrows) or tree (containment line)."""
        cx, cy = (via[0] + self.dx, via[1]) if via else (None, None)
        p0 = border(a, *(via and (cx, cy) or (b['x'], b['y'])), gap=1)
        p1 = border(b, *(via and (cx, cy) or (a['x'], a['y'])), gap=0 if kind == 'tree' else 3)
        d = f'M{p0[0]:.1f},{p0[1]:.1f} ' + (f'Q{cx},{cy} ' if via else 'L') + f'{p1[0]:.1f},{p1[1]:.1f}'
        marker = {'edge': 'n', 'thin': 't', 'bad': 'b', 'good': 'g'}.get(kind)
        tip = f' marker-end="url(#{marker})"' if marker else ''
        self.back.append(f'  <path class="{kind}" d="{d}"{tip}/>\n')

    def svg(self):
        return ''.join(self.back + self.front)


def border(n, tx, ty, gap):
    dx, dy = tx - n['x'], ty - n['y']
    hw, hh = n['w'] / 2 + gap, n['h'] / 2 + gap
    t = min(hw / abs(dx) if dx else math.inf, hh / abs(dy) if dy else math.inf)
    return n['x'] + dx * t, n['y'] + dy * t


def write(name, label, before, after):
    svg = (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 800 {HEIGHT}" width="800" height="{HEIGHT}" role="img" aria-label="{label}">\n'
           + STYLE + FRAME + '  <!-- Before -->\n' + before.svg() + '  <!-- After -->\n' + after.svg() + '</svg>\n')
    open(name + '.svg', 'w').write(svg)


# module-cycle: siblings in a ring of uses -> layered DAG
b = Panel(0)
api, socket, files = b.node(200, 85, 'api'), b.node(320, 160, 'socket'), b.node(270, 260, 'files')
kinds, paths = b.node(130, 260, 'kinds'), b.node(80, 160, 'paths')
for s, t in [(api, socket), (api, paths), (socket, kinds), (kinds, files), (socket, paths), (paths, files)]:
    b.edge(s, t)
b.edge(files, socket, 'bad')
b.edge(files, api, 'bad', via=(200, 190))
a = Panel(400)
api, socket = a.node(200, 75, 'api'), a.node(200, 140, 'socket')
kinds, paths, files = a.node(120, 205, 'kinds'), a.node(280, 205, 'paths'), a.node(200, 275, 'files')
for s, t in [(api, socket), (api, paths), (socket, kinds), (socket, paths), (kinds, files), (paths, files)]:
    a.edge(s, t)
write('module-cycle', 'module-cycle: before, five sibling modules use each other in cycles; after, they form layers and every arrow points down', b, a)

# split-module: two far-apart modules leaning on each other -> one subtree
b = Panel(0)
crate, vm, arch = b.node(200, 75, 'crate'), b.node(110, 145, 'vm'), b.node(290, 145, 'arch')
memory, devices = b.node(65, 220, 'memory', 'badbox'), b.node(160, 220, 'devices')
paging, cpu = b.node(250, 220, 'paging', 'badbox'), b.node(330, 220, 'cpu')
for s, t in [(crate, vm), (crate, arch), (vm, memory), (vm, devices), (arch, paging), (arch, cpu)]:
    b.edge(s, t, 'tree')
b.edge(devices, memory)
b.edge(memory, paging, 'bad', via=(155, 300))
b.edge(paging, memory, 'bad', via=(160, 262))
a = Panel(400)
crate, vm, arch = a.node(200, 75, 'crate'), a.node(110, 145, 'vm'), a.node(290, 145, 'arch')
memory, devices = a.node(65, 210, 'memory'), a.node(160, 210, 'devices')
paging, cpu = a.node(65, 275, 'paging', 'goodbox'), a.node(290, 210, 'cpu')
for s, t in [(crate, vm), (crate, arch), (vm, memory), (vm, devices), (memory, paging), (arch, cpu)]:
    a.edge(s, t, 'tree')
a.edge(devices, memory)
write('split-module', 'split-module: before, vm::memory and arch::paging depend on each other across the tree; after, paging is a child of memory', b, a)

# facade-bypass: outside arrows into a module's insides -> through its facade
def facade_tree(p, child_cls):
    crate = p.node(200, 70, 'crate')
    socket, files, kinds = p.node(70, 145, 'socket'), p.node(160, 145, 'files'), p.node(290, 145, 'kinds')
    epoll, stream, pipe = p.node(220, 240, 'epoll', child_cls), p.node(288, 240, 'stream', child_cls), p.node(350, 240, 'pipe', child_cls)
    for s, t in [(crate, socket), (crate, files), (crate, kinds), (kinds, epoll), (kinds, stream), (kinds, pipe)]:
        p.edge(s, t, 'tree')
    return socket, files, kinds, epoll, stream, pipe
b = Panel(0)
socket, files, kinds, epoll, stream, pipe = facade_tree(b, 'mod')
for s, t in [(socket, epoll), (socket, stream), (files, pipe), (files, epoll)]:
    b.edge(s, t, 'bad')
a = Panel(400)
socket, files, kinds, epoll, stream, pipe = facade_tree(a, 'private')
a.edge(socket, kinds, 'good', via=(170, 215))
a.edge(files, kinds, 'good', via=(215, 185))
write('facade-bypass', 'facade-bypass: before, socket and files reach into kinds submodules; after, they use kinds and its submodules are private', b, a)

# field-leak: callers poking fields -> callers -> methods -> fields
b = Panel(0)
b.box(200, 60, 175, 235, 'RunQueue')
sched, timer, signals = b.node(75, 115, 'scheduler'), b.node(75, 180, 'timer'), b.node(75, 245, 'signals')
items, length, head = b.node(300, 115, 'items', 'badbox'), b.node(300, 180, 'len', 'badbox'), b.node(300, 245, 'head', 'badbox')
for s, t in [(sched, length), (sched, items), (timer, head), (timer, items), (signals, length)]:
    b.edge(s, t, 'bad')
a = Panel(400)
a.box(195, 60, 180, 235, 'RunQueue')
sched, timer, signals = a.node(75, 115, 'scheduler'), a.node(75, 180, 'timer'), a.node(75, 245, 'signals')
push, pop = a.node(245, 130, 'push()', 'goodbox'), a.node(245, 230, 'pop()', 'goodbox')
items, length, head = a.node(337, 105, 'items', 'private'), a.node(337, 180, 'len', 'private'), a.node(337, 255, 'head', 'private')
for s, t in [(sched, push), (signals, push), (timer, pop)]:
    a.edge(s, t, 'good')
for s, t in [(push, items), (push, length), (pop, length), (pop, head)]:
    a.edge(s, t, 'thin')
write('field-leak', 'field-leak: before, three modules access RunQueue fields directly; after, they call push and pop, which own the fields', b, a)

# chatty-seam: many-to-many calls across a seam -> few calls, tree behind them
INNER = ['alloc', 'map', 'pin', 'prot', 'flush', 'unmap']
def app_side(p):
    p.box(20, 55, 105, 245, 'app')
    return p.node(72, 115, 'boot()'), p.node(72, 180, 'restore()'), p.node(72, 245, 'reset()')
b = Panel(0)
boot, restore, reset = app_side(b)
b.box(190, 55, 185, 245, 'memory')
inner = [b.node(290, 95 + 36 * i, n + '()') for i, n in enumerate(INNER)]
for s, ts in [(boot, [0, 1, 3, 4]), (restore, [0, 1, 2, 4]), (reset, [2, 3, 4, 5])]:
    for t in ts:
        b.edge(s, inner[t], 'bad')
a = Panel(400)
boot, restore, reset = app_side(a)
a.box(190, 55, 185, 245, 'memory')
load, clear = a.node(238, 131, 'load()', 'goodbox'), a.node(238, 239, 'reset()', 'goodbox')
inner = [a.node(330, 95 + 36 * i, n + '()', 'private') for i, n in enumerate(INNER)]
for s, t in [(boot, load), (restore, load), (reset, clear)]:
    a.edge(s, t, 'good')
for s, ts in [(load, [0, 1, 2]), (clear, [3, 4, 5])]:
    for t in ts:
        a.edge(s, inner[t], 'thin')
write('chatty-seam', 'chatty-seam: before, app functions call many memory functions; after, they call two memory operations', b, a)

# shallow-module: callers hitting most functions -> few entry points over a deep tree
def callers(p):
    return p.node(90, 80, 'app'), p.node(200, 80, 'server'), p.node(310, 80, 'cli')
b = Panel(0)
b.box(25, 125, 350, 170, 'config')
app, server, cli = callers(b)
row1 = [b.node(x, 175, cls='badbox', w=32, h=20) for x in (60, 125, 190, 255, 320)]
row2 = [b.node(x, 245, cls=c, w=32, h=20) for x, c in zip((92, 157, 222, 287, 340),
                                                          ('badbox', 'mod', 'badbox', 'mod', 'badbox'))]
for s, ts in [(app, [row1[0], row1[1], row2[0]]), (server, [row1[1], row1[2], row1[3], row2[2]]),
              (cli, [row1[3], row1[4], row2[4]])]:
    for t in ts:
        b.edge(s, t, 'bad')
a = Panel(400)
a.box(25, 125, 350, 170, 'config')
app, server, cli = callers(a)
entry = [a.node(x, 165, cls='goodbox', w=32, h=20) for x in (130, 270)]
mid = [a.node(x, 215, cls='private', w=32, h=20) for x in (90, 170, 230, 310)]
low = [a.node(x, 262, cls='private', w=32, h=20) for x in (90, 170, 230, 310)]
for s, t in [(app, entry[0]), (server, entry[0]), (server, entry[1]), (cli, entry[1])]:
    a.edge(s, t, 'good')
for s, t in [(entry[0], mid[0]), (entry[0], mid[1]), (entry[1], mid[2]), (entry[1], mid[3])] + list(zip(mid, low)):
    a.edge(s, t, 'thin')
write('shallow-module', 'shallow-module: before, callers use 8 of 10 functions; after, they use 2 entry points over a tree of hidden functions', b, a)

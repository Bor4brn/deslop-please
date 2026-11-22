"""Draws the README figures, one before/after SVG per lint. Run from this directory: python3 figures.py"""
import math
from html import escape

STYLE = '''  <style>
    text { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif; font-size: 13px; fill: #1f2328; }
    .mono { font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; font-size: 12px; }
    .small { font-size: 12px; }
    .code { font-size: 11px; }
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
            self.front.append(f'  <text class="mono{TEXT_CLASS.get(cls, "")}" x="{n["x"]:.1f}" y="{y+4}" text-anchor="middle">{escape(label)}</text>\n')
        return n

    def code(self, x, y, title, lines, cls='mod'):
        """A function, named in bold, over the lines of code that matter."""
        w = max(len(t) for t in [title, *lines]) * 7.0 + 20
        h = 22 + 15 * len(lines)
        n = dict(x=x + self.dx, y=y, w=w, h=h)
        top = y - h / 2
        self.front.append(f'  <rect class="{cls}" x="{n["x"]-w/2:.1f}" y="{top:.1f}" width="{w:.1f}" height="{h}" rx="5"/>\n')
        self.front.append(f'  <text class="mono bold{TEXT_CLASS.get(cls, "")}" x="{n["x"]:.1f}" y="{top+16:.1f}" text-anchor="middle">{escape(title)}</text>\n')
        for i, line in enumerate(lines):
            self.front.append(f'  <text class="mono code muted" x="{n["x"]:.1f}" y="{top+31+15*i:.1f}" text-anchor="middle">{escape(line)}</text>\n')
        return n

    def note(self, x, y, text, anchor='start'):
        self.front.append(f'  <text class="small muted" x="{x+self.dx}" y="{y}" text-anchor="{anchor}">{escape(text)}</text>\n')

    def box(self, x, y, w, h, label, cls='box'):
        """A module; `badbox` when the report names it."""
        self.back.append(f'  <rect class="{cls}" x="{x+self.dx}" y="{y}" width="{w}" height="{h}" rx="6"/>\n')
        self.back.append(f'  <text class="mono{TEXT_CLASS.get(cls, " muted")}" x="{x+self.dx+10}" y="{y+18}">{escape(label)}</text>\n')
        return dict(x=x + self.dx + w / 2, y=y + h / 2, w=w, h=h)

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


# Red fill marks what the report names: modules in module-cycle, things in tangle,
# fields in field-leak, shown functions in shallow-module. Each Before caption is
# the report; each After caption is the fix.

# module-cycle: kinds and socket use each other, though their things only form a
# chain listen -> Stream::new -> BACKLOG_MOST -> move BACKLOG_MOST to its user
b = Panel(0)
b.box(25, 70, 155, 215, 'kinds', 'badbox')
b.box(215, 70, 160, 215, 'socket', 'badbox')
new = b.code(102, 180, 'Stream::new', ['<= BACKLOG_MOST'])
listen, most = b.code(295, 125, 'listen()', ['Stream::new(..)']), b.node(295, 240, 'BACKLOG_MOST')
b.edge(listen, new)
b.edge(new, most)
b.note(375, 35, '2 modules use each other in a circle', 'end')
a = Panel(400)
a.box(25, 70, 155, 215, 'kinds')
a.box(215, 70, 160, 215, 'socket')
new, most = a.code(102, 150, 'Stream::new', ['<= BACKLOG_MOST']), a.node(102, 240, 'BACKLOG_MOST', 'goodbox')
listen = a.code(295, 125, 'listen()', ['Stream::new(..)'])
a.edge(listen, new, 'good')
a.edge(new, most, 'thin')
a.note(375, 35, 'BACKLOG_MOST moved to where it is used', 'end')
write('module-cycle', 'module-cycle: before, modules kinds and socket use each other, though listen, Stream::new and BACKLOG_MOST only form a chain; after, BACKLOG_MOST moves into kinds and only socket uses kinds', b, a)

# tangle: Tasks and Timers know each other across kernel and its child timers,
# with no module cycle -> tick in kernel calls both (variants lifted, lifted_untied)
def kernel(p):
    p.box(25, 60, 350, 235, 'kernel')
    p.box(190, 100, 178, 175, 'timers')
b = Panel(0)
kernel(b)
tasks = b.code(105, 215, 'Tasks', ['timers.arm(id, 10)'], 'badbox')
timers = b.code(279, 200, 'Timers', ['tasks.wake(id)'], 'badbox')
b.edge(tasks, timers, 'bad', via=(192, 145))
b.edge(timers, tasks, 'bad', via=(192, 278))
b.note(375, 35, '2 things know each other, across 2 modules', 'end')
a = Panel(400)
kernel(a)
tick = a.code(105, 112, 'tick()', ['timers.fire(now)', 'tasks.wake(id)'], 'goodbox')
tasks = a.code(105, 225, 'Tasks', ['timers.arm(id, 10)'])
timers = a.code(279, 200, 'Timers', ['fire(now) -> Vec<u32>'])
a.edge(tick, tasks, 'good')
a.edge(tick, timers, 'good')
a.edge(tasks, timers, 'thin')
a.note(375, 35, 'Timers no longer knows Tasks', 'end')
write('tangle', 'tangle: before, Tasks in kernel and Timers in its child module timers know each other, with no module cycle; after, tick in kernel calls both and Timers no longer knows Tasks', b, a)

# field-leak: the sched functions work on RunQueue's fields -> they call RunQueue's methods
def sched(p, code):
    p.box(18, 52, 174, 134, 'sched::policy')
    p.box(18, 192, 174, 106, 'sched::report')
    return [p.code(105, y, name, lines) for y, (name, lines) in zip((98, 154, 234, 276), code)]
b = Panel(0)
admit, next_, backlog, drained = sched(b, [('admit', ['items.push(task)']),
                                           ('next', ['items.get(head)', 'head += 1']),
                                           ('backlog', ['items.len() - head']),
                                           ('drained', ['head == items.len()'])])
b.box(222, 52, 155, 246, 'RunQueue')
items, head = b.node(300, 125, 'items', 'badbox'), b.node(300, 214, 'head', 'badbox')
b.node(300, 268, 'limit', 'private')
for s_ in (admit, next_, backlog):
    b.edge(s_, items, 'bad')
b.edge(drained, items, 'bad', via=(212, 262))
b.edge(next_, head, 'bad')
for s_ in (backlog, drained):
    b.edge(s_, head, 'thin')
b.note(375, 35, 'grey: a read, no leak', 'end')
a = Panel(400)
admit, next_, backlog, drained = sched(a, [('admit', ['queue.push(task)']),
                                           ('next', ['queue.pop()']),
                                           ('backlog', ['queue.backlog()']),
                                           ('drained', ['queue.backlog() == 0'])])
a.box(207, 52, 170, 246, 'RunQueue')
push, pop, back = a.node(256, 110, 'push()', 'goodbox'), a.node(256, 175, 'pop()', 'goodbox'), a.node(256, 245, 'backlog()', 'goodbox')
limit, items, head = a.node(343, 95, 'limit', 'private'), a.node(343, 175, 'items', 'private'), a.node(343, 255, 'head', 'private')
for s_, t in [(admit, push), (next_, pop), (backlog, back), (drained, back)]:
    a.edge(s_, t, 'good')
for s_, t in [(push, items), (push, limit), (pop, items), (pop, head), (back, items), (back, head)]:
    a.edge(s_, t, 'thin')
write('field-leak', 'field-leak: before, functions in sched push to, index and bump the items and head of RunQueue; after, they call push, pop and backlog, and only RunQueue works on its fields', b, a)

# shallow-module: one type that shows most of its functions -> a few entry points over hidden ones
STORE = ['new', 'insert', 'get', 'remove', 'contains', 'room', 'is_full', 'total', 'largest']
TABLE = ['new', 'count', 'put', 'find', 'value', 'take']
def store(p):
    p.box(108, 55, 270, 242, 'store')
    p.box(282, 80, 88, 210, 'Table')
    for i, f in enumerate(TABLE):
        p.node(326, 118 + 30 * i, f, 'private', w=58, h=20)
b = Panel(0)
store(b)
app = b.node(55, 175, 'app')
b.note(196, 92, 'Store', 'middle')
shown = [b.node(196, 104 + 22 * i, f'{f}()', 'badbox', w=96, h=18) for i, f in enumerate(STORE)]
for t in shown:
    b.edge(app, t, 'bad', via=(125, t['y']))
b.note(375, 35, 'store shows 9 of its 15 functions', 'end')
a = Panel(400)
store(a)
app = a.node(55, 140, 'app')
a.note(196, 92, 'Store', 'middle')
entry = [a.node(196, 110 + 30 * i, f'{f}()', 'goodbox', w=96, h=20) for i, f in enumerate(['new', 'load', 'summary'])]
for i, f in enumerate(STORE[1:]):
    a.node(160 + 72 * (i % 2), 205 + 24 * (i // 2), f, 'private', w=66, h=19)
for t in entry:
    a.edge(app, t, 'good')
a.note(375, 35, 'store shows 3 of its 15 functions', 'end')
write('shallow-module', 'shallow-module: before, app calls 9 of the 15 functions of store; after, it calls new, load and summary, and the rest are hidden behind them', b, a)

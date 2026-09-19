#!/usr/bin/env python3
"""Render the sealed Zettai campaign summary; no fitted or synthetic results.

Chart map: two relationship panels compare endpoint count with strong-scaling
speedup and weak-scaling makespan. Both require curves to show saturation.
Blue/gold/neutral identify compute/link profiles; markers also identify them.
The data retains gate/serial controls, record costs, run identities and queue time.
"""
import argparse
import json
import statistics
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
COUNTS = [1, 2, 4, 8, 16, 32, 64, 128, 256]
PROFILES = ['compute', 'balanced', 'link']


def load(path):
    s = json.loads(path.read_text())
    assert s['schema'] == 'slugarch.zettai256.v1' and s['status'] == 'pass'
    assert s['fresh_processes'] == 693 and s['checked_rounds'] == 2052
    assert s['injected_failures'] == 45 and s['rejected_offline_mutations'] == 10
    assert len(s['groups']) == 228 and s['max_endpoints'] == 256
    identities = set()
    for g in s['groups']:
        assert len(g['runs']) == 3
        for r in g['runs']:
            assert r['run_id'] not in identities
            identities.add(r['run_id'])
        values = [r['duration_ns'] for r in g['runs']]
        assert g['median_ns'] == statistics.median(values)
        assert g['min_ns'] == min(values) == max(values) == g['max_ns']
    assert len(identities) == 684
    return s


def group(s, n, profile, scaling, enforce=True, schedule='concurrent', record_ns=64):
    selection = dict(n=n, profile=profile, scaling=scaling, enforce=enforce,
                     schedule=schedule, record_ns=record_ns)
    found = [g for g in s['groups'] if all(g['config'][k] == v for k, v in selection.items())]
    assert len(found) == 1
    return found[0]


def macros(s):
    values = dict(ZettaiIntegratedProcesses=f"{816+s['fresh_processes']:,}",
                  ZettaiProcesses=str(s['fresh_processes']),
                  ZettaiRounds=f"{s['checked_rounds']:,}",
                  ZettaiFaults=str(s['injected_failures']),
                  ZettaiChecks=f"{s['checks']:,}",
                  ZettaiBridgeChecks=f"{s['bridge_disable_restore_checks']:,}")
    for profile in PROFILES:
        a = group(s, 1, profile, 'strong')['median_ns']
        b = group(s, 256, profile, 'strong')['median_ns']
        values[f'Zettai{profile.capitalize()}Speedup'] = f'{a/b:.2f}'
    return '% Generated from the validated Zettai summary by plot_zettai_results.py\n' + ''.join(
        f'\\newcommand{{\\{k}}}{{{v}}}\n' for k, v in values.items())


def plot(s, output):
    import matplotlib
    matplotlib.use('Agg')
    import matplotlib.pyplot as plt
    from matplotlib.lines import Line2D
    from matplotlib.ticker import ScalarFormatter
    plt.rcParams.update({'font.family': 'DejaVu Sans', 'font.size': 8,
                         'axes.titlesize': 9, 'pdf.fonttype': 42})
    fig, axes = plt.subplots(1, 2, figsize=(7.12, 2.9))
    colors = ['#28649A', '#B17422', '#666666']
    markers = ['o', 's', '^']
    labels = ['Compute: c=32, b=32', 'Balanced: c=8, b=16', 'Link: c=1, b=2']
    for profile, color, marker in zip(PROFILES, colors, markers):
        for enforce, style in [(False, '--'), (True, '-')]:
            strong = [group(s, n, profile, 'strong', enforce)['median_ns'] for n in COUNTS]
            weak = [group(s, n, profile, 'weak', enforce)['median_ns']/1e6 for n in COUNTS]
            for ax, ys in zip(axes, [[strong[0]/v for v in strong], weak]):
                ax.plot(COUNTS, ys, color=color, linestyle=style, marker=marker,
                        markerfacecolor=color if enforce else 'white', markersize=3,
                        linewidth=1.15)
    axes[0].plot(COUNTS, COUNTS, color='#999999', linewidth=.8, linestyle=':')
    axes[0].set_title('(a) Strong: 4,096 words total', loc='left')
    axes[0].set_ylabel('Virtual-time speedup vs. one endpoint')
    axes[0].set_yscale('log', base=2)
    axes[0].set_yticks([1, 4, 16, 64, 256])
    axes[0].yaxis.set_major_formatter(ScalarFormatter())
    axes[0].set_ylim(.8, 330)
    axes[1].set_title('(b) Weak: 4,096 words/endpoint', loc='left')
    axes[1].set_ylabel('Three-round virtual makespan (ms)')
    axes[1].set_yscale('log')
    axes[1].set_yticks([.1, 1, 10])
    axes[1].yaxis.set_major_formatter(ScalarFormatter())
    axes[1].set_ylim(.045, 16)
    for ax in axes:
        ax.set_xscale('log', base=2)
        ax.set_xticks(COUNTS, [str(n) for n in COUNTS], fontsize=7)
        ax.set_xlabel('Type-2 endpoint instances through Zettai')
        ax.spines[['top', 'right']].set_visible(False)
        ax.grid(axis='y', color='#dddddd', linewidth=.5, which='major')
        ax.minorticks_off()
    handles = [Line2D([0], [0], color=c, marker=m, label=l, linewidth=1.15,
                      markersize=3) for c, m, l in zip(colors, markers, labels)]
    fig.legend(handles=handles, loc='upper center', ncol=3, frameon=False,
               bbox_to_anchor=(.51, 1.01), columnspacing=1.4, fontsize=8)
    fig.text(.51, .862, 'Assumed c: ns/word; b: bytes/ns · solid: gate on; dashed: off · 3 runs/point',
             ha='center', fontsize=7.3)
    fig.subplots_adjust(left=.083, right=.985, bottom=.175, top=.75, wspace=.36)
    output.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(output.with_suffix('.pdf'), metadata={'CreationDate': None, 'ModDate': None})
    fig.savefig(output.with_suffix('.png'), dpi=220)
    plt.close(fig)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--data', type=Path, default=ROOT/'docs/evaluation/slugarch-zettai256-20260919.json')
    ap.add_argument('--output', type=Path, default=ROOT/'docs/images/slugarch-zettai256')
    ap.add_argument('--macros', type=Path)
    ap.add_argument('--check', action='store_true')
    args = ap.parse_args()
    s = load(args.data)
    if args.macros:
        if args.check:
            assert args.macros.read_text() == macros(s)
        else:
            args.macros.write_text(macros(s))
    if not args.check:
        plot(s, args.output)
    print('Zettai summary and derived numbers: PASS')


if __name__ == '__main__':
    main()

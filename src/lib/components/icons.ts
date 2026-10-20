// Icons from the mockups (docs/design/_build/build.py ICONS), as data for Icon.svelte. 16×16 grid.
export interface IconNode {
	t: 'path' | 'rect' | 'circle' | 'g';
	a: Record<string, string>;
	c?: IconNode[];
}

export const ICONS = {
	terminal: [
		{
			t: 'rect',
			a: {
				x: '2',
				y: '3',
				width: '12',
				height: '10',
				rx: '1.5'
			}
		},
		{
			t: 'path',
			a: {
				d: 'M5 6.8l2 1.6-2 1.6M8.5 10.4H11'
			}
		}
	],
	chat: [
		{
			t: 'path',
			a: {
				d: 'M2.8 4h10.4v6.6H8.2L5 13.2v-2.6H2.8z'
			}
		}
	],
	desktop: [
		{
			t: 'rect',
			a: {
				x: '2',
				y: '3',
				width: '12',
				height: '8',
				rx: '1'
			}
		},
		{
			t: 'path',
			a: {
				d: 'M6 13.5h4M8 11v2.5'
			}
		}
	],
	lock: [
		{
			t: 'rect',
			a: {
				x: '4',
				y: '7',
				width: '8',
				height: '6',
				rx: '1'
			}
		},
		{
			t: 'path',
			a: {
				d: 'M5.6 7V5.6a2.4 2.4 0 0 1 4.8 0V7'
			}
		}
	],
	check: [
		{
			t: 'path',
			a: {
				d: 'M3.5 8.5l3 3 6-7'
			}
		}
	],
	cross: [
		{
			t: 'path',
			a: {
				d: 'M4.5 4.5l7 7M11.5 4.5l-7 7'
			}
		}
	],
	dot: [
		{
			t: 'circle',
			a: {
				cx: '8',
				cy: '8',
				r: '3.6',
				fill: 'currentColor',
				stroke: 'none'
			}
		}
	],
	ring: [
		{
			t: 'circle',
			a: {
				cx: '8',
				cy: '8',
				r: '4.2'
			}
		}
	],
	magnifier: [
		{
			t: 'circle',
			a: {
				cx: '7',
				cy: '7',
				r: '3.8'
			}
		},
		{
			t: 'path',
			a: {
				d: 'M9.8 9.8L13.2 13.2'
			}
		}
	],
	stamp: [
		{
			t: 'g',
			a: {
				class: 'stamp'
			},
			c: [
				{
					t: 'circle',
					a: {
						cx: '8',
						cy: '8',
						r: '6.2'
					}
				},
				{
					t: 'circle',
					a: {
						cx: '8',
						cy: '8',
						r: '4.4',
						'stroke-width': '0.9'
					}
				},
				{
					t: 'path',
					a: {
						d: 'M5.9 8.1l1.5 1.5 2.8-3.1'
					}
				}
			]
		}
	],
	question: [
		{
			t: 'path',
			a: {
				d: 'M6 6.1a2.1 2.1 0 1 1 2.8 2c-.5.2-.8.6-.8 1.1v.5'
			}
		},
		{
			t: 'circle',
			a: {
				cx: '8',
				cy: '12',
				r: '.7',
				fill: 'currentColor',
				stroke: 'none'
			}
		}
	],
	copy: [
		{
			t: 'rect',
			a: {
				x: '5.5',
				y: '5.5',
				width: '8',
				height: '8',
				rx: '1'
			}
		},
		{
			t: 'path',
			a: {
				d: 'M3.5 10.5v-6a1 1 0 0 1 1-1h6'
			}
		}
	],
	paste: [
		{
			t: 'rect',
			a: {
				x: '3.5',
				y: '3.5',
				width: '9',
				height: '10.5',
				rx: '1'
			}
		},
		{
			t: 'rect',
			a: {
				x: '6',
				y: '2',
				width: '4',
				height: '3',
				rx: '.8'
			}
		}
	],
	chev: [
		{
			t: 'path',
			a: {
				d: 'M6 4l4 4-4 4'
			}
		}
	],
	'chev-down': [
		{
			t: 'path',
			a: {
				d: 'M4 6l4 4 4-4'
			}
		}
	],
	plus: [
		{
			t: 'path',
			a: {
				d: 'M8 3.5v9M3.5 8h9'
			}
		}
	],
	warn: [
		{
			t: 'path',
			a: {
				d: 'M8 2.6L14.2 13.2H1.8z'
			}
		},
		{
			t: 'path',
			a: {
				d: 'M8 6.8v2.8'
			}
		},
		{
			t: 'circle',
			a: {
				cx: '8',
				cy: '11.4',
				r: '.7',
				fill: 'currentColor',
				stroke: 'none'
			}
		}
	],
	info: [
		{
			t: 'circle',
			a: {
				cx: '8',
				cy: '8',
				r: '6'
			}
		},
		{
			t: 'path',
			a: {
				d: 'M8 7.3v3.8'
			}
		},
		{
			t: 'circle',
			a: {
				cx: '8',
				cy: '5.2',
				r: '.7',
				fill: 'currentColor',
				stroke: 'none'
			}
		}
	],
	sidebar: [
		{
			t: 'rect',
			a: {
				x: '2',
				y: '3',
				width: '12',
				height: '10',
				rx: '1.5'
			}
		},
		{
			t: 'path',
			a: {
				d: 'M6 3v10'
			}
		}
	],
	inspector: [
		{
			t: 'rect',
			a: {
				x: '2',
				y: '3',
				width: '12',
				height: '10',
				rx: '1.5'
			}
		},
		{
			t: 'path',
			a: {
				d: 'M10 3v10'
			}
		}
	],
	drawer: [
		{
			t: 'rect',
			a: {
				x: '2',
				y: '3',
				width: '12',
				height: '10',
				rx: '1.5'
			}
		},
		{
			t: 'path',
			a: {
				d: 'M2 9.5h12'
			}
		}
	],
	search: [
		{
			t: 'circle',
			a: {
				cx: '7',
				cy: '7',
				r: '4'
			}
		},
		{
			t: 'path',
			a: {
				d: 'M10 10l3.5 3.5'
			}
		}
	],
	sliders: [
		{
			t: 'path',
			a: {
				d: 'M2.5 5h11M2.5 11h11'
			}
		},
		{
			t: 'circle',
			a: {
				cx: '6',
				cy: '5',
				r: '1.7',
				class: 'knob'
			}
		},
		{
			t: 'circle',
			a: {
				cx: '10.5',
				cy: '11',
				r: '1.7',
				class: 'knob'
			}
		}
	],
	clock: [
		{
			t: 'circle',
			a: {
				cx: '8',
				cy: '8',
				r: '5.6'
			}
		},
		{
			t: 'path',
			a: {
				d: 'M8 5v3.2l2.1 1.3'
			}
		}
	],
	log: [
		{
			t: 'path',
			a: {
				d: 'M5.5 4.5h8M5.5 8h8M5.5 11.5h8'
			}
		},
		{
			t: 'circle',
			a: {
				cx: '3',
				cy: '4.5',
				r: '.6',
				fill: 'currentColor'
			}
		},
		{
			t: 'circle',
			a: {
				cx: '3',
				cy: '8',
				r: '.6',
				fill: 'currentColor'
			}
		},
		{
			t: 'circle',
			a: {
				cx: '3',
				cy: '11.5',
				r: '.6',
				fill: 'currentColor'
			}
		}
	],
	pulse: [
		{
			t: 'path',
			a: {
				d: 'M1.8 8.5h3l1.6-4 3 7.2 1.6-3.2h3.2'
			}
		}
	],
	folder: [
		{
			t: 'path',
			a: {
				d: 'M2.5 4.2h4l1.5 1.5h5.5v6.8h-11z'
			}
		}
	],
	tray: [
		{
			t: 'path',
			a: {
				d: 'M2.5 9.5l2-6h7l2 6v3.5h-11z'
			}
		},
		{
			t: 'path',
			a: {
				d: 'M2.5 9.5h3l1 1.5h3l1-1.5h3'
			}
		}
	],
	hatch: [
		{
			t: 'rect',
			a: {
				x: '3',
				y: '3',
				width: '10',
				height: '10'
			}
		},
		{
			t: 'path',
			a: {
				d: 'M3 8l5-5M3 13l10-10M8 13l5-5'
			}
		}
	],
	dashsq: [
		{
			t: 'rect',
			a: {
				x: '3',
				y: '3',
				width: '10',
				height: '10',
				'stroke-dasharray': '2 1.6'
			}
		}
	],
	open: [
		{
			t: 'path',
			a: {
				d: 'M5 11l6-6M6 5h5v5'
			}
		}
	],
	restore: [
		{
			t: 'path',
			a: {
				d: 'M5 4.5L2.5 7 5 9.5'
			}
		},
		{
			t: 'path',
			a: {
				d: 'M2.5 7H10a3.2 3.2 0 0 1 0 6.4H7.5'
			}
		}
	],
	wrench: [
		{
			t: 'path',
			a: {
				d: 'M10.2 2.8a3 3 0 0 0-3.6 4l-4 4a1.2 1.2 0 0 0 1.7 1.7l4-4a3 3 0 0 0 4-3.6l-1.8 1.8-1.7-.4-.4-1.7z'
			}
		}
	],
	import: [
		{
			t: 'path',
			a: {
				d: 'M8 2.5v7.5M5 7l3 3 3-3M3 13.5h10'
			}
		}
	],
	branch: [
		{
			t: 'circle',
			a: {
				cx: '5',
				cy: '3.8',
				r: '1.5'
			}
		},
		{
			t: 'circle',
			a: {
				cx: '5',
				cy: '12.2',
				r: '1.5'
			}
		},
		{
			t: 'circle',
			a: {
				cx: '11',
				cy: '5.8',
				r: '1.5'
			}
		},
		{
			t: 'path',
			a: {
				d: 'M5 5.3v5.4M11 7.3c0 2.3-2.4 2.6-5.4 3.8'
			}
		}
	],
	external: [
		{
			t: 'path',
			a: {
				d: 'M9 3h4v4M13 3L7.5 8.5M11.5 9.5V13H3V4.5h3.5'
			}
		}
	],
	skip: [
		{
			t: 'path',
			a: {
				d: 'M4 4l5 4-5 4zM11.5 4v8'
			}
		}
	],
	play: [
		{
			t: 'path',
			a: {
				d: 'M5 3.5l7.5 4.5L5 12.5z'
			}
		}
	],
	pen: [
		{
			t: 'path',
			a: {
				d: 'M10.8 2.8l2.4 2.4-7.6 7.6-3 .6.6-3z'
			}
		}
	],
	file: [
		{
			t: 'path',
			a: {
				d: 'M4 2.5h5l3 3v8H4z'
			}
		},
		{
			t: 'path',
			a: {
				d: 'M9 2.5v3h3'
			}
		}
	],
	link: [
		{
			t: 'path',
			a: {
				d: 'M7 9a2.6 2.6 0 0 0 3.7 0l2-2a2.6 2.6 0 0 0-3.7-3.7l-.8.8M9 7a2.6 2.6 0 0 0-3.7 0l-2 2A2.6 2.6 0 0 0 7 12.7l.8-.8'
			}
		}
	],
	stop: [
		{
			t: 'rect',
			a: {
				x: '4',
				y: '4',
				width: '8',
				height: '8',
				rx: '1'
			}
		}
	],
	broken: [
		{
			t: 'path',
			a: {
				d: 'M7 9a2.6 2.6 0 0 0 3.7 0l2-2a2.6 2.6 0 0 0-3.7-3.7M9 7a2.6 2.6 0 0 0-3.7 0l-2 2A2.6 2.6 0 0 0 7 12.7M2.5 2.5l2 2M11.5 11.5l2 2'
			}
		}
	]
} as const satisfies Record<string, readonly IconNode[]>;

export type IconName = keyof typeof ICONS;

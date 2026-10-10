# zks-backbeat-packager

This is my personal packager - a thing that turns rhythm game charts into `.bbzip` files.

## What is this?

This is a tool that turns rhythm game charts into `.bb` files, or more usefully, `.bbzip` files.

It also lets you import existing rhythm game charts into your backbeat store. It does this by turning them into `.bbzip` files and then importing them. What do you expect :P.

`.bbzip` files are a useful unit for transferring backbeat-packaged content around on the internet, for example, [makiba](https://makiba.ac) accepts it as an upload method.

## Why is this not official?

Yes I'm the person that made backbeat, but backbeat doesn't ship any game-specific logic. Backbeat doesn't know anything about `.bms` files or `.ksh` files etc.

One huge advantage of this is that backbeat therefore works for _any_ rhythm game, past, present or future. If KSH gets updated to have new functionality, backbeat doesn't care.

## What is a packager?

A packager needs to read a rhythm game file and perform the following steps:

- Fracturing

If this chart format contains "multiple" charts (e.g. `.sm`), you **MUST** break them into their individual charts. `.bb` files are only for one playable chart.
This requirement becomes more obvious when you think about tables - when a table references your "many charts in one .bb file" thing, how is it meant to know which chart to pick?
**Therefore, I cannot understate how important this is:** do not put multiple playable charts in the `chart` field of a `.bb` file. You must fracture it.

- Describe

You must be able to format a human readable "description" of the chart you're packaging. This is usually something like
`Artist - Song Title (charter's Difficulty Name)`.

This is used in the UI to tell users what they don't have installed.

- Resolving

You must be able to traverse every "referenced" asset from the chart file, and then `sha256` those files.

If you're writing a ksh packager for example, you need to be able to parse this chart:

```ksh
title=Apollo
artist=Xelloscope
effect=11ne
jacket=Apollo.jpg
illustrator=A00
difficulty=extended
level=17
t=77.5-155
m=Apollo.ogg
mvol=80
o=67
bg=space
layer=techno
...
```

and correctly work out that this chart references the following files:

```
Apollo.jpg
Apollo.ogg
```

And then work out the sha256 for both of those files:
```json
{
  "Apollo.jpg": "f73163e2ce2822fe9865eb7b14d410e24c46609837db5a272d315198454b69c7",
  "Apollo.ogg": "6bebff3e23c06e2ef63222a7a11e174224e91b31fb875df735ccd6ec9f1ec4b7"
}
```

Finally, you need to be able to use `gzip` and `base64` to compress the chart itself. The final output will look like this:

```
{
	"filename": "exh.ksh",
	"assets": {
		"Apollo.jpg": "f73163e2ce2822fe9865eb7b14d410e24c46609837db5a272d315198454b69c7",
		"Apollo.ogg": "6bebff3e23c06e2ef63222a7a11e174224e91b31fb875df735ccd6ec9f1ec4b7"
	},
	"desc": "Xelloscope - Apollo (11ne's EXH)",
	"chart": "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"
}
```

Congrats.

# Catholic Version

Looking to fork and extend to include deuterocanonical books.

Look for Catholic Public Domain Version (CPDV)
Pros: It provides a modern reading level. If you want your application to feel clean, contemporary, and readable without violating copyrights on translations like the RSV-2CE or NABRE, this is the only text that fits that description. The underlying XML schema structure across public codebases is typically very well-behaved.

Cons: It is the work of a single individual, not a committee of theologians or linguists. Because it was independently published on the internet without a Vatican Imprimatur, serious Catholic users or academics generally do not use it for study or devotion, viewing it more as an internet novelty or developer proxy.

<https://get.bible/bible-data-sets/>
<https://github.com/scrollmapper/bible_databases_deuterocanonical>
[Catholic Public Domain Version in USFM/PSFM format](https://github.com/BibleCorps/ENG-B-CPDV2009-pd-PSFM)

## USFM vs PSFM

Both **USFM** and **PSFM** are specialized, lightweight markup languages specifically designed to digitize, format, and translate scripture. They are the industry standards maintained by **UBS** (United Bible Societies) and **SIL International**.

If you are looking at files inside a Bible repository, you will almost always see these formats used to manage raw text before it gets converted into HTML, JSON, or mobile apps.

Here is a breakdown of what they are and how they work.

---

### 1. USFM (Unified Scripture Format Markup)

USFM is a **tag-based text format** that looks a bit like a cross between Markdown and legacy LaTeX or HTML. It uses backward slashes (`\`) followed by a marker name to define structural elements (chapters, verses, paragraphs, poetic lines, footnotes).

#### Why does it exist?

Translators and developers needed a format that could be easily read and typed in a standard text editor without the massive, bloated syntax of heavy XML or JSON. Because it uses plain text, it plays incredibly well with version control systems like **Git**.

#### What it looks like

```usfm
\id GEN
\c 1
\p
\v 1 In the beginning God created the heaven and the earth.
\v 2 And the earth was without form, and void;
\q1
\v 3 And God said, Let there be light:
\q2 and there was light.

```

#### Key Markers

* `\id`: The standard 3-letter book code (e.g., `GEN` for Genesis, `MAT` for Matthew, `TOB` for Tobit).
* `\c`: Chapter number.
* `\p`: Standard paragraph block.
* `\v`: Verse number and text.
* `\q1`, `\q2`: Poetic indentation levels (vital for Psalms and prophets).

---

### 2. PSFM (Paratext Scripture Format Markup)

PSFM is a highly optimized variant of USFM used specifically by **Paratext**, the dominant software program used globally by Bible translation teams.

For an application developer, **USFM and PSFM are functionally identical.** They share the same marker rules and structure. The main difference is that PSFM includes tight validation rules and strict system metadata markers specifically designed to prevent translators from making formatting mistakes during data entry.

> **Developer Note:** If you download a data set and it contains `.psfm` files, you can usually rename the extension to `.usfm` or treat it identically in your parsing regex/logic.

---

### How to use them in your stack

If you are pulling USFM/PSFM files down from a source like `Get.Bible` or a GitHub repo, you don't actually want to display them to users in their raw format. You want to parse them.

You have three excellent choices for handling this data:

1. **Use an existing Parser:** Do not write a regex parser from scratch if you can avoid it. The open-source ecosystem has highly robust libraries like `usfm-parser` (JavaScript/TypeScript) or Python equivalents that instantly ingest a `.usfm` file and output a perfectly structured, nested JSON object.
2. **Convert to OSIS XML:** Many legacy web engines convert USFM into OSIS (Open Scripture Information Standard), which is a formalized XML schema that uses traditional strict tags (e.g., `<chapter id="Gen.1">`).
3. **Direct JSON Transformation:** Most modern web apps use a build script to parse the USFM files into a local SQLite database or flat JSON structures mapped by `Book -> Chapter -> Verse` arrays, which your frontend can then dynamically query.

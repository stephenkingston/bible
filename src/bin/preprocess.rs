// preprocess.rs all XML bibles into binary of the `Bible` struct type
// and write to disc. This will enable us to quickly load the binary
// version of the Bible at runtime. The output binary files are in the
// src/bibles/ folder

// These paths are relative to repo root
const SOURCE_FOLDER: &str = "xml/";
const OUTPUT_FOLDER: &str = "src/bibles";

use bible::{Bible, Book, BookName, Chapter, Edition, Verse, util::filestem_to_edition};
use std::path::PathBuf;
use walkdir::WalkDir;

fn parse_xml_to_bible_struct(path: &PathBuf) -> Bible {
    let filestem = path.file_stem().unwrap().to_str().unwrap();
    let edition: Edition = filestem_to_edition(filestem);

    let mut bible = Bible {
        books: Vec::new(),
        edition,
    };

    println!("Parsing {:?}", path);
    let file = std::fs::File::open(path).unwrap();
    let root = xmltree::Element::parse(file).expect(format!("Failed to parse {:?}", path).as_str());

    for testament_xml in root.children {
        // root.children -> Old Testament, New Testament
        for book_xml in testament_xml.as_element().unwrap().children.iter() {
            // child.children -> Book
            let book_name = BookName::from_index(
                book_xml
                    .as_element()
                    .unwrap()
                    .attributes
                    .get("number")
                    .unwrap()
                    .parse()
                    .expect("Failed to parse book index"),
            );
            if book_name.is_none() {
                panic!("Found invalid book index in {:?}", path);
            } else {
                let book_name = book_name.unwrap();
                let mut book = Book {
                    name: book_name,
                    chapters: Vec::new(),
                };
                for chapter_xml in book_xml.as_element().unwrap().children.iter() {
                    // child.children -> Chapter
                    let chapter_number = chapter_xml
                        .as_element()
                        .unwrap()
                        .attributes
                        .get("number")
                        .unwrap()
                        .parse()
                        .expect("Failed to parse chapter index");

                    let mut chapter = Chapter {
                        number: chapter_number,
                        verses: Vec::new(),
                    };

                    for verse_xml in chapter_xml.as_element().unwrap().children.iter() {
                        // child.children -> Verses
                        let verse_number = verse_xml
                            .as_element()
                            .unwrap()
                            .attributes
                            .get("number")
                            .unwrap()
                            .parse()
                            .expect("Failed to parse verse index");

                        let verse_text = verse_xml.as_element().unwrap().get_text().unwrap();

                        chapter.verses.push(Verse {
                            number: verse_number,
                            text: verse_text.to_string(),
                        });
                    }

                    // Chapter done, push into Book
                    book.chapters.push(chapter);
                }

                // Book done, push into Bible
                bible.books.push(book);
            }
        }
    }

    bible
}

fn main() {
    // Walk through all source files in SOURCE_FOLDER
    let src_xml_files: Vec<PathBuf> = WalkDir::new(SOURCE_FOLDER)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|entry| {
            entry.path().is_file()
                && entry.path().extension().and_then(|s| s.to_str()) == Some("xml")
        })
        .map(|entry| entry.path().to_path_buf())
        .collect();

    for file in src_xml_files {
        let bible = parse_xml_to_bible_struct(&file);

        // Write to output folder with the same file name (.bin), overwrite any existing file
        let outfile = PathBuf::from(OUTPUT_FOLDER)
            .join(
                file.file_stem()
                    .expect(format!("Failed to create path for {:?}", file).as_str()),
            )
            .with_extension("bin");

        let bincode_config = bincode::config::standard();
        let bin = bincode::encode_to_vec(bible, bincode_config)
            .expect(format!("Failed to encode {:?}", file).as_str());

        std::fs::write(&outfile, bin).expect(format!("Failed to write {:?}", outfile).as_str());
    }
}

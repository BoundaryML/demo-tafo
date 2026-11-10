// Shared whitespace normalization; preserves punctuation and Unicode.
function whitespace_words(text: string) -> string[] throws never {
    text.chars()
        .map((character) -> {
            if (character.is_whitespace()) {
                " "
            } else {
                character
            }
        })
        .join("")
        .split(" ")
        .filter((word) -> {
            word != ""
        })
}


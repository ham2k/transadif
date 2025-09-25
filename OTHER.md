### Mojibake

Mojibake means a sequence of characters encoded as an 8-bit character set,
but that happen to coincide with UTF-8 extended sequences and should be interpreted as UTF-8. For example the character 'é' is encoded as 0xEA in ISO-8859-1, but in UTF-8 it is encoded as 0xC3 0xA9. Those to bytes correspond to "Ã©" when interpreted as ISO-8859-1. So if we read an ISO string but find a sequence that corresponds to UTF-8, we should consider it mojibake and can replace it with the corresponding Unicode character.

These misencodings can also be happen repeatedly, so "Ã©" gets interpreted as UTF-8 without correction, and then represented as ISO-88591-1 without transcoding, generating the sequence of bytes 0xC3 0x83 0xC2 0xA9, which in ISO-8859-1 corresponds to "ÃÂ©".


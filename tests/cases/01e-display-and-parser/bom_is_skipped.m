% covers: 9 - a leading UTF-8 byte-order mark is skipped, not lexed
% QA D29. The three bytes EF BB BF open this file, ahead of the marker, which
% is the shape a Windows editor or PowerShell produces. Today they lex as
% `unexpected character '\u{feff}'` and the script never runs. The marker is
% therefore not the first byte of the file, so this case is discovered by its
% sibling .out rather than by the marker.
disp(1)

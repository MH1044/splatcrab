% covers: 4 - (Design notes, the SVG vocabulary) a label's &, <, > and " are escaped and a control character is replaced, so the text is still the whole content of one <text> and the file stays well formed
% The title holds an ESC (char 27), which XML 1.0 forbids; it must not
% reach the file. The file is named after the case, read back and deleted
% before anything is asserted.
plot(1:2);
xlabel('a<b & "c"');
title(['x' char(27) 'y']);
saveas(gcf, 'labels_escaped_in_svg.svg');
s = fileread('labels_escaped_in_svg.svg');
delete('labels_escaped_in_svg.svg');
fprintf('%d %d %d\n', ~isempty(strfind(s, '>a&lt;b &amp; &quot;c&quot;<')), isempty(strfind(s, 'a<b')), isempty(strfind(s, char(27))))

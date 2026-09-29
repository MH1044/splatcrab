% covers: 10 - print -dpng writes a PNG file: its first eight bytes are the PNG signature
% Item 10 as the spec writes it, the file named after the case. The file is
% closed and deleted before its bytes are displayed.
plot(1:3);
print('-dpng', 'print_png_signature.png');
fid = fopen('print_png_signature.png');
b = fread(fid, 8);
fclose(fid);
delete('print_png_signature.png');
disp(b')

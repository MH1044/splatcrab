% covers: 13 - print with a resolution too large for any image is a clean error from check_shape, exit 1, and leaves no file behind
% The image's pixel size goes through check_shape before a pixel is
% allocated, so -r100000 is refused. The .err pins check_shape's whole
% text: 437500x583333 is the 560x420 figure at 100000/96 pixels a figure
% pixel, rounded, rows first. The catch shows that no file was written
% (fopen gives -1), deletes one if it was, and raises the error again; the
% print and the rethrow share one line, so either is reported as line 9.
plot(1:3);
try, print('-dpng', '-r100000', 'err_print_huge_resolution.png'); catch e, fid = fopen('err_print_huge_resolution.png'); disp(fid); if fid >= 0, fclose(fid); delete('err_print_huge_resolution.png'); end, rethrow(e); end
delete('err_print_huge_resolution.png');

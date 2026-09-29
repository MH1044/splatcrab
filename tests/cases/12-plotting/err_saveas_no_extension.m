% covers: 1 - (Design notes, error texts) saveas to a file name with no extension and no format is refused with a message naming the fix, before any file is written, exit 1
% The format comes from the extension or from a third argument; with
% neither there is none to write, and the message says how to give one
% rather than calling an empty extension an unsupported format. The catch
% shows that no file was written (fopen gives -1), deletes one if it was,
% and raises the error again; the saveas and the rethrow share one line,
% so the reported line is the same whichever of the two it names.
plot(1:3);
try, saveas(gcf, 'err_saveas_no_extension'); catch e, fid = fopen('err_saveas_no_extension'); disp(fid); if fid >= 0, fclose(fid); delete('err_saveas_no_extension'); end, rethrow(e); end

% covers: 18 - a char element is a UTF-16 code unit (QA D37)
% The emoji is outside the BMP, so it is a surrogate pair: two elements, and
% output decodes the pair back to one UTF-8 character. é is in the BMP: one.
disp(length('😀'))
fprintf('%d %d\n', double('😀'))
disp('😀')
disp(length('é'))

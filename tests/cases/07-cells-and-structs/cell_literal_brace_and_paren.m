% covers: 1 - a cell literal holds values of any class: braces read an element's content, a chain indexes into it, and parentheses index the cell itself, giving a cell
c = {1, 'two', [3 4]}; disp(class(c)); disp(c{2}); disp(c{3}(2)); disp(size(c(2:3)))

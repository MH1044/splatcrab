% covers: 7 - indexed field assignment builds a struct array, p(2).name reads one element's field, and [p.name] concatenates the cs-list p.name
p(1).name = 'A'; p(2).name = 'B'; disp(numel(p)); disp(p(2).name); disp(class(p)); q = [p.name]; disp(q)

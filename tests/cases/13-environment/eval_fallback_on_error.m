% covers: 4 - eval(code, fallback) runs the fallback when the code fails, with lasterr set
eval('error(''boom'')', 'disp(lasterr)')

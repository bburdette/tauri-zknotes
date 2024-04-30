
cdir=$(pwd)
cd ../../zknotes/elm
elm-live src/Main.elm -d "$cdir/static" -h 0.0.0.0 --port 8080 -- --output="$cdir/static/main.js" 

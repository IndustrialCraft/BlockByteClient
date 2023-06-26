rm -rf gen_assets
mkdir gen_assets

cp assets/icon.png gen_assets
cp assets/font.ttf gen_assets
cp assets/breaking*.png gen_assets

unzip ../BlockByteServer/client_content.zip -d gen_assets
cargo run --release -- ./gen_assets localhost:4321
## FOR MY OWN USE ONLY CURRENTLY

How to run the filter/balancer
change config for balancing and filtering in config.rs
Create filter.exe 
cd to executable filter.exe


cargo run --bin filter --input "/nnue/data/run1/*.txt" --output "C:/Users/warre/RustroverProjects/bullet/data/run1.txt"
or from exe

.\filter.exe --input "C:\Users\warre\RustroverProjects\FastPeaPea\nnue\data\run8\*.txt" --output "C:\Users\warre\RustroverProjects\bullet\data\run8.txt"

### Convert to bullet format

Now we have to convert to bullet format and .data files + validation.

cd C:\Users\warre\RustroverProjects\bullet

Open Developer Command Prompt for VS 2022

cargo run --release --bin bullet-utils -- convert --from text --input data/run1.txt --output data/run1.data

To validate :

cargo run --release --bin bullet-utils -- validate --input data/run1.data

Now load in prev network .bin file.

### Train the network

First edit config then run:

cargo r -r --example Pea_simple

cargo r -r --example Pea_output_buckets
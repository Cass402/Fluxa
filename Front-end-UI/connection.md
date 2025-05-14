# Setting Up Fluxa DEX: Local Development Guide

This guide will walk you through setting up a local Solana development environment, deploying the Fluxa AMM Core program, connecting the frontend, and testing the DEX functionalities.

## 1. Setting Up the Local Solana Environment

**Step 1: Verify Solana CLI Installation**

First, check if you have the Solana CLI installed:

```bash
solana --version
If it's not installed, run:sh -c "$(curl -sSfL [https://release.solana.com/v1.17.0/install](https://release.solana.com/v1.17.0/install))"
Add Solana to your PATH if prompted. You may need to restart your terminal.Step 2: Configure Solana for Local DevelopmentSet your Solana configuration to use the local network:solana config set --url localhost
Step 3: Start the Local ValidatorStart the Solana test validator in a terminal window:solana-test-validator --reset
This command starts a local Solana network on your machine. The --reset flag ensures a clean state. Keep this terminal window running in the background.2. Deploying the Fluxa AMM Core ProgramStep 1: Verify Anchor InstallationCheck if Anchor is installed:anchor --version
If it's not installed, run:cargo install --git [https://github.com/coral-xyz/anchor](https://github.com/coral-xyz/anchor) avm --locked
avm install latest
avm use latest
Step 2: Navigate to Your Project Directorycd /Users/siddharth/Desktop/sid_personal/Projects/Fluxa
Step 3: Build the Solana Programanchor build
This command compiles your Solana program and generates necessary files.Step 4: Deploy the Programanchor deploy
This deploys your compiled program to the local Solana network. Note the program ID that Anchor prints after successful deployment, for example:Program ID: 7rcCBu5R2WNSKHzf5mZCGzQMqBamyLcvNM3qpSohKPpo
This ID is crucial for the frontend to communicate with your program.3. Configuring the Frontend ConnectionStep 1: Update the Frontend ConfigurationOpen the config file:vim /Users/siddharth/Desktop/sid_personal/Projects/Fluxa/Front-end-UI/lib/config.ts
Update the following constants with your program ID and local network URL:// Update the PROGRAM_ID with the value from anchor deploy
export const PROGRAM_ID = "7rcCBu5R2WNSKHzf5mZCGzQMqBamyLcvNM3qpSohKPpo"; // Replace with your program ID
// Ensure the network points to your local validator
export const SOLANA_NETWORK = "[http://127.0.0.1:8899](http://127.0.0.1:8899)";
Step 2: Set Up Test Tokens (Optional)If you need test tokens for the DEX, run the setup script:cd /Users/siddharth/Desktop/sid_personal/Projects/Fluxa
zsh scripts/setup-test-accounts.sh
This creates test tokens and funds your wallet.Step 3: Update the IDL FileThe Anchor build process generates an IDL file that describes your program's interface. Copy this to your frontend:cp target/idl/amm_core.json /Users/siddharth/Desktop/sid_personal/Projects/Fluxa/Front-end-UI/services/idl.json
4. Running the FrontendStep 1: Install Frontend DependenciesNavigate to the frontend directory:cd /Users/siddharth/Desktop/sid_personal/Projects/Fluxa/Front-end-UI
Install dependencies:npm install
Step 2: Start the Development Servernpm run dev
The frontend should now be running at http://localhost:3000.5. Testing UI FunctionalitiesConnect a WalletOpen your browser and navigate to http://localhost:3000Click the "Connect Wallet" button in the top-right cornerSelect a wallet provider (e.g., Phantom)Authorize the connection in your walletVerify that your wallet address appears in the UIBackend Verification:Open browser developer tools (F12) and check network requests to confirm the wallet connectionVerify in the Console that the WalletContext state shows your connected walletCreate a Liquidity PoolNavigate to the "Pools" section and click "Create Pool"Select a token pair (e.g., SOL and USDC)Choose a fee tier (e.g., 0.3%)Set your initial price range and deposit amountsReview the details and click "Create"Sign the transaction in your walletBackend Verification: Check the created pool account:solana account <Pool_Address> --output json
You can find the pool address in the transaction details or browser network requests.Add Liquidity to an Existing PoolNavigate to the "Pools" page and select an existing poolClick "Add Liquidity"Define your price range and input the token amountsClick "Add" and sign the transactionBackend Verification: Check your position data:solana account <Position_Address> --output json
Perform a Token SwapNavigate to the "Swap" pageSelect input and output tokensEnter the amount to swapReview the rate and price impactClick "Swap" and sign the transactionBackend Verification:Check your token balances in the UIVerify the transaction in the browser's dev tools Network tabCheck that the appropriate accounts were updated on-chain:solana account <Your_Wallet_Token_Account> --output json
View Your PositionsNavigate to the "Dashboard" or "Positions" pageReview your open positions, including:Token pairsLiquidity rangesCurrent valueStatus (in/out of range)Backend Verification: Compare the displayed data with on-chain position data:solana account <Position_Address> --output json
Collect Trading FeesGo to your positions listFind a position with uncollected feesClick "Collect Fees"Sign the transactionBackend Verification: Check that fees were transferred to your wallet:solana account <Your_Token_Account> --output json
6. Inspecting Backend DataUsing Solana CLITo inspect account data:solana account <ADDRESS> --output json
To list recent transactions:solana transaction-history <YOUR_WALLET_ADDRESS>
Using Browser Developer ToolsOpen Developer Tools (F12)Go to the Network tabFilter by "Fetch/XHR"Look for requests to your local Solana nodeExamine the request payloads and responsesUsing the Program LogsIn the terminal running the validator, you'll see detailed logs for each transaction. These logs show:Transaction execution stepsAccount data changesError messages if transactions fail7. TroubleshootingProgram ID Not FoundDouble-check that the PROGRAM_ID in your frontend configuration matches the ID from anchor deployVerify the program was successfully deployed with:solana program show --programs
Transaction FailuresEnsure your wallet has sufficient SOL for transaction feesCheck the validator logs for specific error messagesMake sure account addresses in your transaction are correctInsufficient BalanceFund your wallet on the local network:solana airdrop 10 <YOUR_WALLET_ADDRESS>
Data Not Displaying CorrectlyCheck network requests to ensure data is being fetched properlyVerify that account data formats match what the frontend expectsEnsure the TickBitmapUtils implementation in the frontend is correctly deserializing the binary tick bitmap dataConnection IssuesMake sure your validator is runningVerify that SOLANA_NETWORK points to `http://
```

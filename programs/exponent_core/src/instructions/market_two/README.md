# Init market steps

Every operation with a market requires interacting with the SY program. Trades withdraw SY tokens (or deposit them). And these deposits/withdrawals must stage any emissions the SY program has produced.

And so the market must have a record for accounts used in these CPI calls. And an Address Lookup Table that stores & compresses these accounts.

Initializing a market takes several steps, which cannot all fit in a single instruction:

- Create the Address Lookup Table
- Extend the Address Lookup Table with all the needed accounts for the CPI calls
- Create the Market itself, which requires
  - Creating a market-owned robot account with the SY program
  - Create a tracker account for recording the total amount of emissions earned by the reward account

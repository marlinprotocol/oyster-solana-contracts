import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { MarketV } from "../target/types/market_v";
import { createAssociatedTokenAccount, createMint, getAccount, getOrCreateAssociatedTokenAccount, mintTo, TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import { assert, expect } from "chai";
import { BankrunProvider, startAnchor } from "anchor-bankrun";
import { Clock, start } from "solana-bankrun";
// import { OysterSolanaContracts } from "../target/types/oyster_solana_contracts";

function get_seeds(seed_str: any): any {
    return [...seed_str].map((char) => char.codePointAt());
}

describe("market_v1", () => {
    // Configure the client to use the local cluster.
    const provider = anchor.AnchorProvider.env();
    anchor.setProvider(provider);

    // add test to initialize the MarketV program
    const program = anchor.workspace.MarketV as Program<MarketV>;
    const authority = provider.wallet.publicKey;
    // (provider.wallet as anchor.Wallet).payer

    let tokenMint: PublicKey;
    let waitTime = new anchor.BN(60);

    // let tokenMint;
    before(async () => {
        // Create a mock USDC mint
        tokenMint = await createMint(
            provider.connection,
            (provider.wallet as anchor.Wallet).payer,
            authority,
            null,
            6 // USDC typically has 6 decimal places
        );
    })

    it("can initialize", async () => {
        const RATE_LOCK_SELECTOR = "RATE_LOCK";
        const tx = await program.methods.initialize(
            RATE_LOCK_SELECTOR,
            waitTime,
            authority,
        ).accounts({
            admin: authority,
            tokenMint,
        }).rpc();
        console.log("Your transaction signature", tx);

        let marketAccount: PublicKey;
        [marketAccount,] = PublicKey.findProgramAddressSync(
            [Buffer.from("market")],
            program.programId
        );
        // check the market account
        let marketData = await program.account.market.fetch(marketAccount);

        expect(marketData.admin.toBase58()).to.equal(authority.toBase58());
        expect(marketData.jobIndex.toString()).to.equal(new anchor.BN(0).toString());
        expect(marketData.tokenMint.toBase58()).to.equal(tokenMint.toBase58());
    });
});

describe("market_v1 - add provider", () => {
    let providerAccount: PublicKey;
    let program: Program<MarketV>;
    let authority: PublicKey;

    before(async () => {
        // Configure the client to use the local cluster.
        const provider = anchor.AnchorProvider.env();
        anchor.setProvider(provider);

        // add test to initialize the MarketV program
        program = anchor.workspace.MarketV as Program<MarketV>;
        authority = provider.wallet.publicKey;
        [providerAccount,] = PublicKey.findProgramAddressSync(
            [Buffer.from("provider"), authority.toBuffer()],
            program.programId
        );
        console.log("providerAccount", providerAccount.toBase58());
    })

    it("can add provider", async () => {
        let cp = "https://example.com/";
        await program.methods.providerAdd(
            cp
        ).accountsStrict({
            provider: providerAccount,
            authority,
            systemProgram: SystemProgram.programId,
        }).rpc();

        const provider = await program.account.provider.fetch(providerAccount);

        expect(provider.cp).to.equal(cp);
    });

    it("cannot add provider with empty cp", async () => {
        let cp = "";
        // check for fail transaction
        try {
            await program.methods.providerAdd(
                cp
            ).accountsStrict({
                provider: providerAccount,
                authority,
                systemProgram: SystemProgram.programId,
            }).signers([]).rpc();
        } catch (error) {
            console.log("error: ", error?.error);
            expect((error as anchor.AnchorError).message).to.be.an('ProviderAlreadyExists');
        }
    });

});

describe("market_v1 - remove provider", () => {
    let providerAccount: PublicKey;
    let program: Program<MarketV>;
    let authority: PublicKey;

    before(async () => {
        // Configure the client to use the local cluster.
        const provider = anchor.AnchorProvider.env();
        anchor.setProvider(provider);

        // add test to initialize the MarketV program
        program = anchor.workspace.MarketV as Program<MarketV>;
        authority = provider.wallet.publicKey;
        [providerAccount,] = PublicKey.findProgramAddressSync(
            [Buffer.from("provider"), authority.toBuffer()],
            program.programId
        );
        console.log("providerAccount", providerAccount.toBase58());

        let cp = "https://example.com/";
        await program.methods.providerAdd(
            cp
        ).accountsStrict({
            provider: providerAccount,
            authority,
            systemProgram: SystemProgram.programId,
        }).rpc();
    })

    it("can remove provider", async () => {
        await program.methods.providerRemove().accountsStrict({
            provider: providerAccount,
            authority,
            // systemProgram: SystemProgram.programId,
        }).rpc();

        // verify that the provider is removed
        try {
            await program.account.provider.fetch(providerAccount);
            assert.fail("The provider account should have been deleted");
        } catch (error) {
            expect(error.message).to.include("Account does not exist");
        }
    });

    it("cannot remove provider if not the authority", async () => {
        // Create a new provider account
        // let providerAccount1: PublicKey;
        // [providerAccount1,] = PublicKey.findProgramAddressSync(
        //     [Buffer.from("provider"), authority.toBuffer()],
        //     program.programId
        // );

        // check for fail transaction
        await program.methods.providerRemove().accounts({
            // provider: providerAccount1,
            authority,
            // systemProgram: SystemProgram.programId,
        }).rpc();
        // try {
        //     await program.methods.providerRemove().accounts({
        //         // provider: providerAccount1,
        //         authority,
        //         // systemProgram: SystemProgram.programId,
        //     }).rpc();
        // } catch (error) {
        //     console.log("error: ", error?.error);
        //     expect((error as anchor.AnchorError).message).to.be.an('ProviderDoesNotExist');
        // }
    });

});

describe("market_v1 - update provider", () => {
    let providerAccount: PublicKey;
    let program: Program<MarketV>;
    let authority: PublicKey;

    before(async () => {
        // Configure the client to use the local cluster.
        const provider = anchor.AnchorProvider.env();
        anchor.setProvider(provider);

        // add test to initialize the MarketV program
        program = anchor.workspace.MarketV as Program<MarketV>;
        authority = provider.wallet.publicKey;
        [providerAccount,] = PublicKey.findProgramAddressSync(
            [Buffer.from("provider"), authority.toBuffer()],
            program.programId
        );
        console.log("providerAccount", providerAccount.toBase58());

        let cp = "https://example.com/";
        await program.methods.providerAdd(
            cp
        ).accountsStrict({
            provider: providerAccount,
            authority,
            systemProgram: SystemProgram.programId,
        }).rpc();
    });

    it("can update cp", async () => {
        let newCp = "https://new-example.com/";
        await program.methods.providerUpdateWithCp(newCp)
            .accounts({
                authority
            })
            .rpc();

        const provider = await program.account.provider.fetch(providerAccount);
        expect(provider.cp).to.equal(newCp);
    });
});

describe("market_v1 - job open", () => {
    let provider: anchor.AnchorProvider;
    let providerAccount: PublicKey;
    let program: Program<MarketV>;
    let authority: PublicKey;
    let tokenMint: PublicKey;
    let authorityTokenAccount: PublicKey;

    before(async () => {
        // Configure the client to use the local cluster.
        provider = anchor.AnchorProvider.env();
        anchor.setProvider(provider);

        // add test to initialize the MarketV program
        program = anchor.workspace.MarketV as Program<MarketV>;
        authority = provider.wallet.publicKey;

        // Create a mock USDC mint
        tokenMint = await createMint(
            provider.connection,
            (provider.wallet as anchor.Wallet).payer,
            authority,
            null,
            6 // USDC typically has 6 decimal places
        );

        // get token account of the owner
        authorityTokenAccount = await createAssociatedTokenAccount(
            provider.connection,
            (provider.wallet as anchor.Wallet).payer,
            tokenMint,
            authority
        );

        // mint tokens to owner
        await mintTo(
            provider.connection,
            (provider.wallet as anchor.Wallet).payer,
            tokenMint,
            authorityTokenAccount,
            authority,
            10 ** 8 // Amount of tokens to mint (in smallest unit, e.g., 1 USDC = 1,000,000 micro USDC)
        );

        const RATE_LOCK_SELECTOR = "RATE_LOCK";

        // initialize the MarketV program
        let waitTime = new anchor.BN(60);
        await program.methods.initialize(
            RATE_LOCK_SELECTOR,
            waitTime,
            authority,
        ).accounts({
            admin: authority,
            tokenMint,
        }).rpc();

        [providerAccount,] = PublicKey.findProgramAddressSync(
            [Buffer.from("provider"), authority.toBuffer()],
            program.programId
        );

        // add provider
        let cp = "https://example.com/";
        await program.methods.providerAdd(
            cp
        ).accounts({
            authority,
        }).rpc();
    });

    it("can open job", async () => {
        const ownerTokenAccountDataInitial = await program.provider.connection.getTokenAccountBalance(authorityTokenAccount);

        let jobIndex = new anchor.BN(1);
        let jobAccount: PublicKey;
        [jobAccount,] = PublicKey.findProgramAddressSync(
            [Buffer.from("job"), Buffer.from(jobIndex.toArray('le', 8))],
            program.programId
        );
        let metadata = "metadata example",
            provider = providerAccount,
            rate = new anchor.BN(10),
            balance = new anchor.BN(100);

        await program.methods.jobOpen(
            metadata,
            provider,
            rate,
            balance
        ).accounts({
            job: jobAccount,
            tokenMint,
            ownerTokenAccount: authorityTokenAccount,
        }).rpc();

        const jobData = await program.account.job.fetch(jobAccount);

        expect(jobData.index.eq(jobIndex)).to.be.true;
        expect(jobData.metadata).to.equal(metadata);
        expect(jobData.provider.toBase58()).to.equal(provider.toBase58());
        expect(jobData.rate.eq(rate)).to.be.true;
        expect(jobData.balance.eq(balance)).to.be.true;

        // check mint token balance of owner
        const ownerTokenAccountDataFinal = await program.provider.connection.getTokenAccountBalance(authorityTokenAccount);
        expect(new anchor.BN(ownerTokenAccountDataInitial.value.amount)
                .sub(new anchor.BN(ownerTokenAccountDataFinal.value.amount)).toString())
            .to.eq(balance.toString());

        // check mint token balance of job token account
        let jobTokenAddress: PublicKey;
        [jobTokenAddress,] = PublicKey.findProgramAddressSync(
            [Buffer.from("job_token"), tokenMint.toBuffer()],
            program.programId
        );
        const jobTokenAccountData = await program.provider.connection.getTokenAccountBalance(jobTokenAddress);
        expect(jobTokenAccountData.value.amount).to.eq(balance.toString());
    });
});

describe.only("market_v1 - job settle", () => {
    let provider: anchor.AnchorProvider;
    // let provider: BankrunProvider;
    let providerAccount: PublicKey;
    let program: Program<MarketV>;
    let authority: PublicKey;
    let tokenMint: PublicKey;
    let providerTokenAccount: PublicKey;

    before(async () => {
        // Configure the client to use the local cluster.
        provider = anchor.AnchorProvider.env();
        // const context = await startAnchor(".", [], []);
        // provider = new BankrunProvider(context);
        anchor.setProvider(provider);

        console.log("HERE1");
        // add test to initialize the MarketV program
        program = anchor.workspace.MarketV as Program<MarketV>;
        console.log("HERE2");
        authority = provider.wallet.publicKey;
        console.log("HERE3");

        // Create a mock USDC mint
        tokenMint = await createMint(
            provider.connection,
            (provider.wallet as anchor.Wallet).payer,
            authority,
            null,
            6 // USDC typically has 6 decimal places
        );
        console.log("HERE4");

        // get token account of the owner
        let authorityTokenAccount = await createAssociatedTokenAccount(
            provider.connection,
            (provider.wallet as anchor.Wallet).payer,
            tokenMint,
            authority
        );

        // mint tokens to owner
        await mintTo(
            provider.connection,
            (provider.wallet as anchor.Wallet).payer,
            tokenMint,
            authorityTokenAccount,
            authority,
            10 ** 8 // Amount of tokens to mint (in smallest unit, e.g., 1 USDC = 1,000,000 micro USDC)
        );

        const RATE_LOCK_SELECTOR = "RATE_LOCK";

        // initialize the MarketV program
        let waitTime = new anchor.BN(60);
        await program.methods.initialize(
            RATE_LOCK_SELECTOR,
            waitTime,
            authority,
        ).accounts({
            admin: authority,
            tokenMint,
        }).rpc();

        // [providerAccount,] = PublicKey.findProgramAddressSync(
        //     [Buffer.from("provider"), authority.toBuffer()],
        //     program.programId
        // );
        // generate random wallet for provider
        const providerWallet = Keypair.generate();
        providerAccount = providerWallet.publicKey;
        // get token account of the provider
        providerTokenAccount = await createAssociatedTokenAccount(
            provider.connection,
            (provider.wallet as anchor.Wallet).payer,
            tokenMint,
            providerAccount
        );
        console.log("HERE5");

        // add provider
        let cp = "https://example.com/";
        await program.methods.providerAdd(
            cp
        ).accounts({
            authority,
        }).rpc();

        let jobIndex = new anchor.BN(1);
        let jobAccount: PublicKey;
        [jobAccount,] = PublicKey.findProgramAddressSync(
            [Buffer.from("job"), Buffer.from(jobIndex.toArray('le', 8))],
            program.programId
        );
        let metadata = "metadata example",
            rate = new anchor.BN(5 * 10 ** 12),
            balance = new anchor.BN(100);

        await program.methods.jobOpen(
            metadata,
            providerAccount,
            rate,
            balance
        ).accounts({
            job: jobAccount,
            tokenMint,
            ownerTokenAccount: authorityTokenAccount,
        }).rpc();
        console.log("HERE6");
    });

    it("can settle job", async () => {
        // const ownerTokenAccountDataInitial = await program.provider.connection.getTokenAccountBalance(authorityTokenAccount);

        // Log the current blockchain time
        let slot = await program.provider.connection.getSlot();
        let blockTime = await program.provider.connection.getBlockTime(slot);
        console.log("Current blockchain time1:", slot, new Date(blockTime * 1000).toISOString());
        // console.log("time: ", await provider.connection.getLatestBlockhashAndContext());

        // const context = await startAnchor(".", [], []);
        // let bankrunProvider = new BankrunProvider(context);
        // anchor.setProvider(bankrunProvider);
        // await bankrunProvider.context.warpToEpoch(BigInt(20000));
        console.log("ProgramId: ", program.programId );
        const context = await start(
            [{ name: "market_v", programId: program.programId }],
            [],
        );
        const currentClock = await context.banksClient.getClock();
        context.setClock(
            new Clock(
                currentClock.slot,
                currentClock.epochStartTimestamp,
                currentClock.epoch,
                currentClock.leaderScheduleEpoch,
                BigInt(50),
            ),
        )

        slot = await program.provider.connection.getSlot();
        blockTime = await program.provider.connection.getBlockTime(slot);
        console.log("Current blockchain time2:", slot, new Date(blockTime * 1000).toISOString());


        console.log("DONE");
        
        // settle job
        let jobIndex = new anchor.BN(1);
        let jobAccount: PublicKey;
        [jobAccount,] = PublicKey.findProgramAddressSync(
            [Buffer.from("job"), Buffer.from(jobIndex.toArray('le', 8))],
            program.programId
        );
        // execute the settle job
        let txn = await program.methods.jobSettle(jobIndex)
            .accounts({
                tokenMint,
                providerTokenAccount,
            }).rpc({commitment: 'confirmed'});

        // get txn timestamp
        let txnReceipt = await program.provider.connection.getParsedTransaction(txn, {commitment: 'confirmed'});
        let txnTimestamp = new anchor.BN(txnReceipt.blockTime);

        const jobData = await program.account.job.fetch(jobAccount);
        let amount = (jobData.rate.mul(txnTimestamp
                                .sub(jobData.lastSettled)
                                .add(new anchor.BN(10 ** 12 - 1))))
                                .div(new anchor.BN(10 ** 12));

        expect(jobData.index.eq(jobIndex)).to.be.true;
        // expect(jobData.provider.toBase58()).to.equal(provider.toBase58());
        // expect(jobData.rate.eq(rate)).to.be.true;
        console.log("data: ", jobData.balance.toNumber(), new anchor.BN(100).sub(amount).toNumber());
        // expect(jobData.balance).to.eq(new anchor.BN(100).sub(amount));

        // // check mint token balance of owner
        // const ownerTokenAccountDataFinal = await program.provider.connection.getTokenAccountBalance(authorityTokenAccount);
        // expect(new anchor.BN(ownerTokenAccountDataInitial.value.amount)
        //         .sub(new anchor.BN(ownerTokenAccountDataFinal.value.amount)).toString())
        //     .to.eq(balance.toString());

        // // check mint token balance of job token account
        // let jobTokenAddress: PublicKey;
        // [jobTokenAddress,] = PublicKey.findProgramAddressSync(
        //     [Buffer.from("job_token"), tokenMint.toBuffer()],
        //     program.programId
        // );
        // const jobTokenAccountData = await program.provider.connection.getTokenAccountBalance(jobTokenAddress);
        // expect(jobTokenAccountData.value.amount).to.eq(balance.toString());
    });
});

describe("market_v1 - job close", () => {
    let providerAccount: PublicKey;
    let program: Program<MarketV>;
    let authority: PublicKey;
    let tokenMint: PublicKey;
    let authorityTokenAccount: PublicKey;

    before(async () => {
        // Configure the client to use the local cluster.
        const provider = anchor.AnchorProvider.env();
        anchor.setProvider(provider);

        // add test to initialize the MarketV program
        program = anchor.workspace.MarketV as Program<MarketV>;
        authority = provider.wallet.publicKey;

        // Create a mock USDC mint
        tokenMint = await createMint(
            provider.connection,
            (provider.wallet as anchor.Wallet).payer,
            authority,
            null,
            6 // USDC typically has 6 decimal places
        );

        // get token account of the owner
        authorityTokenAccount = await createAssociatedTokenAccount(
            provider.connection,
            (provider.wallet as anchor.Wallet).payer,
            tokenMint,
            authority
        );

        // mint tokens to owner
        await mintTo(
            provider.connection,
            (provider.wallet as anchor.Wallet).payer,
            tokenMint,
            authorityTokenAccount,
            authority,
            10 ** 8 // Amount of tokens to mint (in smallest unit, e.g., 1 USDC = 1,000,000 micro USDC)
        );

        const RATE_LOCK_SELECTOR = "RATE_LOCK";

        // initialize the MarketV program
        let waitTime = new anchor.BN(60);
        await program.methods.initialize(
            RATE_LOCK_SELECTOR,
            waitTime,
            authority,
        ).accounts({
            admin: authority,
            tokenMint,
        }).rpc();

        [providerAccount,] = PublicKey.findProgramAddressSync(
            [Buffer.from("provider"), authority.toBuffer()],
            program.programId
        );

        // add provider
        let cp = "https://example.com/";
        await program.methods.providerAdd(
            cp
        ).accounts({
            authority,
        }).rpc();

        let jobIndex = new anchor.BN(1);
        let jobAccount: PublicKey;
        [jobAccount,] = PublicKey.findProgramAddressSync(
            [Buffer.from("job"), Buffer.from(jobIndex.toArray('le', 8))],
            program.programId
        );
        let metadata = "metadata example",
            rate = new anchor.BN(5 * 10 ** 12),
            balance = new anchor.BN(100);

        await program.methods.jobOpen(
            metadata,
            providerAccount,
            rate,
            balance
        ).accounts({
            job: jobAccount,
            tokenMint,
            ownerTokenAccount: authorityTokenAccount,
        }).rpc();
    });

    it("can close job", async () => {
        const ownerTokenAccountDataInitial = await program.provider.connection.getTokenAccountBalance(authorityTokenAccount);

        // settle job
        let jobIndex = new anchor.BN(1);
        let jobAccount: PublicKey;
        [jobAccount,] = PublicKey.findProgramAddressSync(
            [Buffer.from("job"), Buffer.from(jobIndex.toArray('le', 8))],
            program.programId
        );
        // execute the settle job
        let txn = await program.methods.jobSettle(jobIndex)
            .accounts({
                tokenMint,
                providerTokenAccount: authorityTokenAccount,
            }).rpc({commitment: 'confirmed'});

        // get txn timestamp
        let txnReceipt = await program.provider.connection.getParsedTransaction(txn, {commitment: 'confirmed'});
        let txnTimestamp = new anchor.BN(txnReceipt.blockTime);

        const jobData = await program.account.job.fetch(jobAccount);
        let amount = (jobData.rate.mul(txnTimestamp
                                .sub(jobData.lastSettled)
                                .add(new anchor.BN(10 ** 12 - 1))))
                                .div(new anchor.BN(10 ** 12));

        expect(jobData.index.eq(jobIndex)).to.be.true;
        // expect(jobData.provider.toBase58()).to.equal(provider.toBase58());
        // expect(jobData.rate.eq(rate)).to.be.true;
        console.log("data: ", jobData.balance.toNumber(), new anchor.BN(100).sub(amount).toNumber());
        // expect(jobData.balance).to.eq(new anchor.BN(100).sub(amount));

        // // check mint token balance of owner
        // const ownerTokenAccountDataFinal = await program.provider.connection.getTokenAccountBalance(authorityTokenAccount);
        // expect(new anchor.BN(ownerTokenAccountDataInitial.value.amount)
        //         .sub(new anchor.BN(ownerTokenAccountDataFinal.value.amount)).toString())
        //     .to.eq(balance.toString());

        // // check mint token balance of job token account
        // let jobTokenAddress: PublicKey;
        // [jobTokenAddress,] = PublicKey.findProgramAddressSync(
        //     [Buffer.from("job_token"), tokenMint.toBuffer()],
        //     program.programId
        // );
        // const jobTokenAccountData = await program.provider.connection.getTokenAccountBalance(jobTokenAddress);
        // expect(jobTokenAccountData.value.amount).to.eq(balance.toString());
    });
});
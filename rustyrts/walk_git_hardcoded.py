# %%
## actix/actix-web
from walkers.rustyrts_walker import walk

path = "../projects/git_walk/actix-web"
branch = "master"

commits = [
    ("f5fd6bc49fd0886cf4a1c76de44c259aff7426c9", None, None),
    ("55a2a59906c05f834fe1278fafb8a8dd5c746510", None, None),
    ("3fde3be3d863a42fab6799bdfd4552bcb43d7e8e", None, None),
    ("987067698b11db82f113ae350356cb11d1d7d1f0", None, None),
    ("c659c33919c4880dbe3d220773f20fc6c5b58070", None, None),
    ("4431c8da654f141f564c9715e4d2962d48e0ed69", None, None),
    ("1a0bf32ec76411e6ae017ea680b4dad7db3f0c69", None, None),
    ("d0b5fb18d2f16f42743518363be8b9e0737cee56", None, None),
    ("0b5b463cfa951d96ec6b0167964ef613b0d2b091", None, None),
    ("679f61cf3751ce9ca53ab64b01baef33b83db937", None, None),
    ("c313c003a4b8b3526b33f782996116263cba7140", None, None),
    ("c9fdcc596db0618495ab8611e52b730e829e36e5", None, None),
    ("e4503046de1263148e1b56394144b1828bbfdac0", None, None),
    ("fa78da81569f23accfe7b293be0c022fd01bbdb3", None, None),
    ("7030bf5fe82dc3c23e61a6f0e7a6b89b53a03e6d", None, None),
    ("1716380f0890a1e936d84181effeb63906c1e609", None, None),
    ("0f09415469843eea4000dc48085101dcf8d75e9b", None, None),
    ("f45038bbfe338661f3b958b10c37dd64d3d70650", None, None),
    ("9aab382ea89395fcc627c5375ddd8721cc47c514", None, None),
    ("21a08ca7969e9a08035a4b9e78d8419f3cce3c64", None, None),
    ("df6fde883c17c39ee30b71858a65c36eb0ed71c0", None, None),
    ("162121bf8d51d497565961d73dde22ba1c36f3a4", None, None),
    ("a35804b89f5b08b11304d4fa3e4ca37c9a4f6627", None, None),
    ("a2d4ff157ea981a09d56e4284fd484e88a5c498d", None, None),
    ("2a5215c1d6cce12ff3f4bdc4e7ac73190d4aa9e0", None, None),
    ("679d1cd51360f62fe5f0084893591b6003671091", None, None),
    ("c22a3a71f2b366bf7af6fd0e00e5f150835645c0", None, None),
    ("0bc4ae9158d51e06e4845bc2e0ba987a5e3aae2e", None, None),
    ("de815dd99cfd95e2759080a7d27535b70b6544b4", None, None),
    ("35006e9cae904ac28ffe81e62ac2e0110346121d", None, None)
]

walk(path, branch=branch, commits=commits)

## Not enough disk space, TODO
## %%
### arrow-datafusion
# from walkers.rustyrts_walker import walk
#
# path = "../projects/git_walk/arrow-datafusion"
# branch = "main"
#
# commits = None #[("2a2e147984088d0c224c440279a4f3122b8ad38e", None, None)]
#
# walk(path, branch=branch, commits=commits)
#

# %%
## feroxbuster
from walkers.rustyrts_walker import walk

path = "../projects/git_walk/feroxbuster"
branch = "main"

commits = [
    ("d3561a58236c289e6cb33ac6a5e6e7d881744e53", None, None),
    ("ce7f3b79b8a661fc40a3ba94d836fd0dd048b427", None, None),
    ("7839118379dc515cdbbc1e44253ce8de25ebad68", None, None),
    ("955fb0ee964f120e9f3238b12cf948c906b80778", None, None),
    ("8d11bb1800776680a7bc7d4b23fe60571a282dd8", None, None),
    ("e8f4bbccf4f589e80210c2db7f4f08cb64e9f89d", None, None),
    ("f29cd16616d43f906a3e5d04fbf08a7704399aa3", None, None),
    ("d0cdf5766bf98cf5e99615549523b1b0f9b58d13", None, None),
    ("addf867f59efdee5b473d005f641a68b58a2d4ea", None, None),
    ("5b8090381eca686cf71a7079509e636c4c8417e1", None, None),
    ("3a128df2fc416e7b3a7f3dc827e4dfa7dfc6ef0a", None, None),
    ("0b16f368a4d84527c57922f966326aff90f03058", None, None),
    ("1e4d3802f809e544af8ae9a5de8d452547559301", None, None),
    ("4f5786ddebdd8aff663893331964b5ac2dd5573f", None, None),
    ("4f31ed1847f3f5e3272617ecbd0567c70628d203", None, None),
    ("02b25dc5535b4b7ebf75b24832340a38b0d92531", None, None),
    ("662521af10836c764ffc355a11437d34d2f638f0", None, None),
    ("9881d65cc362f733034d2ceaf486d543e358ec3d", None, None),
    ("4b0b26da0283b9550af65f834b067f310f9209a5", None, None),
    ("ac56225405d2e627b2613ee5aa4ab080e964d225", None, None),
    ("98767596067285a69be6b059846ac0fe168a3188", None, None),
    ("29ad28d3f8d0ce17a943df636b94d6223a9d5bd4", None, None),
    ("b844985528f90691148bb483ec843d81ac40901c", None, None),
    ("ef0b5d37809799fad56ad60cdd5485ef678fb590", None, None),
    ("71efd78f034fa3511272be9ac0dc9f9552743bfb", None, None),
    ("4578630b1309b57eb546d96ed8d2d7b1681c8601", None, None),
    ("7c4bc213a3599e1e44ae7e73dbbf3445c8b2ce66", None, None),
    ("b7ddf7431dc9e2a7a45a14a8f6cfae9ffc5bfb3d", None, None),
    ("16d34bbee0221804f54077bc2797bda2efefe5a3", None, None),
    ("e8f4438a528dba1b8ebf7c435f241f754d28cffd", None, None)
]

walk(path, branch=branch, commits=commits)

# %%
## nushell
from walkers.rustyrts_walker import walk

path = "../projects/git_walk/nushell"
branch = "main"

commits = [
    ("5f48452e3b8da8dd76ddb9adf03e832a3041dac9", None, None),
    ("367f79cb4fc2f281abe608840733f78f664b34a7", None, None),
    ("d8cde2ae8958b4c07f9d16a7540fa679cf3b17c0", None, None),
    ("b8b2737890736a94506a07a504c3c9e029b66498", None, None),
    ("57761149f473067e070dae4b42708372b99c0ff0", None, None),
    ("1086fbe9b5ef588fb6fda5f09133fb72fc2e2fb7", None, None),
    ("beec6588722c96928b58a8bc300f4353220493c1", None, None),
    ("56ce10347e29544a1d0ef2ae70ca6acb4d81a573", None, None),
    ("9259a56a28f1dd3a4b720ad815aa19c6eaf6adce", None, None),
    ("0d031636a9ed1602623b0b2b7468935cbf7f3148", None, None),
    ("68d98fcf24e84dae83fa034b7f38779496c93291", None, None),
    ("e97ba9b74c607b1b23f4bda84a90303bc66b3bed", None, None),
    ("0450cc25e0272c3d1147afc27176daf43e52e83b", None, None),
    ("525ed7653f641b55093e863f3dcd84c6fffdf52f", None, None),
    ("f818193b5333eb44da92ce258059906507bd13dc", None, None),
    ("3db0aed9f739fd929f8dff287fafd60883649c07", None, None),
    ("d885258dc781807e5758ce274eb99c2437c75667", None, None),
    ("7490392eb982fbbc1036bf9356ada84764f601b1", None, None),
    ("266fac910a2a23e309b3c61342aeee9c5ce520aa", None, None),
    ("bd6c550470e11e3346cd6d99eeea9bd1a3b8b7f9", None, None),
    ("83458510a922fa6e52761897f5e2a95b763143ef", None, None),
    ("f93033c20baab05f2a0640cbdf5da644349e4976", None, None),
    ("fb42c94b791c1e89c69daaa11ca467f774d16812", None, None),
    ("2bb367f570502cd26d6b7a81425d9f3b3b9eb432", None, None),
    ("909b7d2160ffbc4289b82c8090a196cb136ec712", None, None),
    ("626b1b99cd667243f4587d83bdef17d1e907b106", None, None),
    ("27d798270b2051e850f1b2ee3fa118d9992b8ea4", None, None),
    ("9009f68e0980120d1a7d07e0ecf9bb637a3a9dcc", None, None),
    ("b907bf355fd921f22c0cd1f4e0677ba7c6814b3b", None, None),
    ("9da2e142b2b39c8a8dc659f461f37661c871dcfa", None, None)
]

walk(path, branch=branch, commits=commits)

## Not enough disk space
## %%
### ockam
# from walkers.rustyrts_walker import walk
#
# path = "../projects/git_walk/ockam"
# branch = "develop"
#
# commits = None #[("24b17f208b9aa18a0cd3d0050d6360837fa5afc2", None, None)]
#
# walk(path, branch=branch, commits=commits)
#

# %%
# rayon
from walkers.rustyrts_walker import walk

path = "../projects/git_walk/rayon"
branch = "master"

commits = [
    ("f0c5a47fca1f2a1ba627aa9ed0514eb0855147c4", None, None),
    ("b98bb23f0595cbf56afba8f641b366a0433f002e", None, None),
    ("16b3310e30e4d8af11ccd0d76b3e319816b892d3", None, None),
    ("2d62dc6b89895fb28210acf0e160669654b970d1", None, None),
    ("e20b4ede19accb0d65494016fa5bfcd46b636e5d", None, None),
    ("b3e6600ca34054d8cefa61fbee4245d1dccd6ba5", None, None),
    ("ed6a5f75c4dc1cb0ac4f59f6f01b38f8a695a777", None, None),
    ("545feac3b8bbe7c0da42cb8a3aa60c17a9b16829", None, None),
    ("edaf85171b8acbe968586b676a309cd46225c472", None, None),
    ("e7ed9a857e23e8b216c97adc54a52830183df40c", None, None),
    ("a7f5c779761b34293bcc4c16e420c4c5fc4a1c57", None, None),
    ("35c032c6714d6fd9876976c5ad6793f8052d7ff0", None, None),
    ("aa6fdd3f1b1b2411f546df16fff9f805e2bfa93a", None, None),
    ("33790dc7eb68a114feacf00547bd60a3c5ad3c3d", None, None),
    ("d2bef7e2d8524011f01b06d79711a11b99b9316e", None, None),
    ("b56c6b8fe89b474cf07380730ea4bb660406a3a6", None, None),
    ("d31d3e3b8ee85db761f283cca25de43a7b7931be", None, None),
    ("d466fd04a75325f7e6a37b1f142181dbb1e20c12", None, None),
    ("4fd13b033424be5eac826571e017b1a008d0bd06", None, None),
    ("a0efb4abb5bc3a2ee0ed812392a2b7386dc2dd38", None, None),
    ("f7d75532fcbc8151b97dec85131e5a0be3db4b4f", None, None),
    ("54758597379d34108d55343cdb4a5f390f7e2ec4", None, None),
    ("faf347a416e8998ace15e795d0af9040cd0ec15e", None, None),
    ("51f9676ac4a743d124ed88878d611a8b475f488f", None, None),
    ("dd91c279709dfbaead9f70b28854ce632d040a0f", None, None),
    ("d41d29ab56d08c9c7836fcacf415e457488b574f", None, None),
    ("a116575410ce3e2e003f10c64c565e85ac5b3893", None, None),
    ("c396338d8c3ebec81dff95965d89673b8cdfab9b", None, None),
    ("08345e14b0cca55fcc9a427ef6ef057e65ab6dee", None, None),
    ("073b79b22f23dc78769521555388d18897b48b26", None, None)
]

walk(path, branch=branch, commits=commits)

# %%
## rust-analyzer
from walkers.rustyrts_walker import walk

path = "../projects/git_walk/rust-analyzer"
branch = "master"

commits = [
    ("04decd5e6b36574ca30369c26481d5a51d739971", None, None),
    ("9a481d1ecf0f11b4a5bd0220eec3fd93c997b033", None, None),
    ("572f1c08b6ba43bdd57c5cb99f79a08ecd821c1c", None, None),
    ("f55be75a17dab2ca23b34c45e7597fe19a5fc8e4", None, None),
    ("3e1e6227ca525f8631e0bff2215fa3de1b4f4cc1", None, None),
    ("12d0970f7e4c4d7f91cccb12525fceea3c4c0669", None, None),
    ("9153e96e88236e2f867dee8f0f291af5cfaf90f4", None, None),
    ("095843119e703a756047dfe25514fdcc93425341", None, None),
    ("6746a08b442d25dc63a90ace1682ebd9ec9b50b8", None, None),
    ("8ed8e4f25abc95d06487c34e0b2b85778aa6a4b4", None, None),
    ("9a4553b833e8fa57a361d934c3498efa485d27c9", None, None),
    ("d96c489120551d36f5bdb362ce313052350821f7", None, None),
    ("461c0cc07af36fc95bafa6d5a8a9d86735fc64ff", None, None),
    ("78d6b88f211cc9faf88815ce7fb1a91546cfce15", None, None),
    ("de7662c852353febce09196199202ee7f6e8e6c3", None, None),
    ("59bd6e2eea151f097a65f2634dc5488b3c272d92", None, None),
    ("43cad21623bc5de59598a565097be9c7d8642818", None, None),
    ("21359c3ab5fc497d11b2c0f0435c7635336a726e", None, None),
    ("81847524702dd7cb1eeae25a53444b325295b129", None, None),
    ("364162f8759a407a06b360e383de5404875e6000", None, None),
    ("d2fd252f9de23d5801b1ca10c067654bf7d6ef4f", None, None),
    ("046ae1d361d8941a664919e7668a65ae735d4a1b", None, None),
    ("30b7e92afaa6dc6b276d60b8e7b47485ca7c2ee3", None, None),
    ("cdd7118cbf23e21c376092b3b2734407004b8dbf", None, None),
    ("0448b7364666ba59b39bbd5564fe8a34b67b8f01", None, None),
    ("32f5276465266522ebc01b8417feeba99bf00f6f", None, None),
    ("3cd57c425a1f7001cc86222f928f53a7114564df", None, None),
    ("a6a052f4078825ba307843df1770c92d96827075", None, None),
    ("a1a7b07ad33b7dcadedc2af26c3a5f8ef3daca27", None, None),
    ("91bbc55eedbc0f6947b69a0158a7b6c81264024e", None, None)
]

walk(path, branch=branch, commits=commits)

# %%
## wasmer
from walkers.rustyrts_walker import walk

path = "../projects/git_walk/wasmer"
branch = "master"

# Feature test-llvm has lead to a compilation error and has therefore not been considered here
commits = [
    ("65265dbd73c01c8660ed79b570ebef9de8e07a2c", "test-singlepass,test-cranelift,test-no-traps",
     "test-singlepass,test-cranelift,test-no-traps"),
    ("6f2957b4b364c0c664d66e40588282ad5e95a785", "test-singlepass,test-cranelift,test-native,test-jit,coverage",
     "test-singlepass,test-cranelift,test-native,test-jit,coverage"),
    ("44339c18fe782c19ad25a2ae63bd11e0cb4fe011", "test-singlepass,test-cranelift,test-native,test-jit,coverage",
     "test-singlepass,test-cranelift,test-native,test-jit,coverage"),
    ("8defffa672cbfcf282ec35262d7baf3939494415", "test-singlepass,test-cranelift", "test-singlepass,test-cranelift"),
    ("c91e545e03d1a24b083d3ea7368ea0d160e45a4b", "test-singlepass,test-cranelift,test-dylib,test-universal,coverage",
     "test-singlepass,test-cranelift,test-dylib,test-universal,coverage"),
    ("94374e4e98e402052c60a97a983336183c811146", "test-singlepass,test-cranelift,test-native,test-jit,coverage",
     "test-singlepass,test-cranelift,test-native,test-jit,coverage"),
    ("499eb499f01f9870bb1bbd8e768d6a6b6a020a2f", "test-singlepass,test-cranelift,test-universal,coverage",
     "test-singlepass,test-cranelift,test-universal,coverage"),
    ("8074253fe24c62a3e6bbd9ea8e808409eaf9bf24", "test-singlepass,test-cranelift,test-universal,coverage",
     "test-singlepass,test-cranelift,test-universal,coverage"),
    ("9353d09cfabd8a297f3184ec9a5a3787b539c607", "test-singlepass,test-cranelift,test-native,test-jit,coverage",
     "test-singlepass,test-cranelift,test-native,test-jit,coverage"),
    ("36419ec7e48a9efecac336bd9cf4b120bc5e13f1", "test-singlepass,test-cranelift,test-universal,coverage",
     "test-singlepass,test-cranelift,test-universal,coverage"),
    ("03b34144bff602c549dcfbee52520d33252d0e04", "test-singlepass,test-cranelift,test-native,test-jit,coverage",
     "test-singlepass,test-cranelift,test-native,test-jit,coverage"),
    ("bb6f483fac97dbad4a85286970c78809cba4abc3", "test-singlepass,test-cranelift,test-dylib,test-universal,coverage",
     "test-singlepass,test-cranelift,test-dylib,test-universal,coverage"),
    ("a91d2474619388f72b682196ccf70d75289e1745", "test-singlepass,test-cranelift,test-universal,coverage",
     "test-singlepass,test-cranelift,test-universal,coverage"),
    ("54b4495b3f9a9e3dd60cc1bf00d20d07eb777bab", "test-singlepass,test-cranelift,test-dylib,test-universal,coverage",
     "test-singlepass,test-cranelift,test-dylib,test-universal,coverage"),
    ("df30d25d3952ce4b04a75c4353fe8f90df987b1e", "test-singlepass,test-cranelift,test-universal,coverage",
     "test-singlepass,test-cranelift,test-universal,coverage"),
    ("66bfb88d8374be4ffdbfae3257bfc30c6bde0c1f", "test-singlepass,test-cranelift,test-universal,coverage",
     "test-singlepass,test-cranelift,test-universal,coverage"),
    ("9e83e8a4663e4f5f8e95a8d823e96b8a6390ec50", "test-singlepass,test-cranelift,test-universal,coverage",
     "test-singlepass,test-cranelift,test-universal,coverage"),
    ("19c4c623a819f3f3bac28af91d749254fa82fcfc", "test-singlepass,test-cranelift,test-universal,coverage",
     "test-singlepass,test-cranelift,test-universal,coverage"),
    ("f8c0910c337a0e571abcf457f42e936aa98e275e", "test-singlepass,test-cranelift,test-universal,coverage",
     "test-singlepass,test-cranelift,test-universal,coverage"),
    ("9ecedc3925f7c764916025c78bdc626cf45ef46b", "test-singlepass,test-cranelift,test-dylib,test-universal,coverage",
     "test-singlepass,test-cranelift,test-dylib,test-universal,coverage"),
    ("c03d61b78a1a0bbd5f542cf1235e88b6a639ee6a", "test-singlepass,test-cranelift,test-native,test-jit,coverage",
     "test-singlepass,test-cranelift,test-native,test-jit,coverage"),
    ("59d065ee32a24e8f82e6c762c4d6ecaa3a481879", "test-singlepass,test-cranelift,test-native,test-jit,coverage",
     "test-singlepass,test-cranelift,test-native,test-jit,coverage"),
    ("88b4093646f60e680dc3fa3be9877c2ad77257a0", "test-singlepass,test-cranelift,test-native,test-jit,coverage",
     "test-singlepass,test-cranelift,test-native,test-jit,coverage"),
    ("1f08f66ba6e8815b92a53fa065a166f82c865c1f", "test-singlepass,test-cranelift,test-native,test-jit,test-no-traps",
     "test-singlepass,test-cranelift,test-native,test-jit,test-no-traps"),
    ("156f0483b3c7868bcf3fb56f84b1c35cd8eb9531", "test-singlepass,test-cranelift,test-universal,coverage",
     "test-singlepass,test-cranelift,test-universal,coverage"),
    ("9ae02d5c0b66bcaff799ad4c64ab70057bc01e09", "test-singlepass,test-cranelift,test-universal,coverage",
     "test-singlepass,test-cranelift,test-universal,coverage"),
    ("1dad3dd1cb6b94b0197938134839cf8fe9fbe9a5", "test-singlepass,test-cranelift,test-universal,coverage",
     "test-singlepass,test-cranelift,test-universal,coverage"),
    ("3d6241236c50bf4f8f378c50abeb84b733116a00", "test-singlepass,test-cranelift,test-universal,coverage",
     "test-singlepass,test-cranelift,test-universal,coverage"),
    ("1dd6d8d3eb4f2633e1f05acce50803302f8a3498", "test-singlepass,test-cranelift,test-native,test-jit,coverage",
     "test-singlepass,test-cranelift,test-native,test-jit,coverage"),
    ("59b37e4b75a5b0d5cd701858defbfbc99dbace13", "test-singlepass,test-cranelift,test-universal,coverage",
     "test-singlepass,test-cranelift,test-universal,coverage")
]

walk(path, branch=branch, commits=commits)

# %%
## zenoh
from walkers.rustyrts_walker import walk

path = "../projects/git_walk/zenoh"
branch = "master"

commits = [
    ("431ffd48f6dd4192f63a168efbd84bcfa7142749", None, None),
    ("472cc34acb9d85f781a1c6fe735fa124c88ce97b", None, None),
    ("0e791bc11d6819f68d143072f7bf1bb60b341ebf", None, None),
    ("f7ff3f4278b96ef60eb14f8907c36d5cc2795dee", None, None),
    ("7ec9427f8cb091387802d6a7e1dd910248941ef8", None, None),
    ("974441cd7addd8bb22ed7a819652fee9115b9d1e", None, None),
    ("85409a54adb7598bf5d07b95ccc4d01a2f0331e3", None, None),
    ("e144f3d502ac69f9ab2a9b612525a7f67b3f2a1e", None, None),
    ("eab33b2a9f6a136f5a2b9823358d58690741b0be", None, None),
    ("26c77ce77dee5f850358df3a696c45690cdfe5e9", None, None),
    ("8b78e6a090cad12a8f0271667025085538cfb389", None, None),
    ("3d94ac1a22268fa06ee0c76cdd385ba1ce454a4a", None, None),
    ("3302c23929e4e66f5aced4df5e03b4eafd8b2f3f", None, None),
    ("2a806883239b1a377e5012b610973562c1127f6e", None, None),
    ("65fed69e8500e066375568f69d1616779a83e509", None, None),
    ("38f7334a9e8fac86f631b254d5e64a5053e5b671", None, None),
    ("0cb17cd9017eb9555819ad9c3d65ccfb10feee0a", None, None),
    ("059c5bf3cfca916a3dbaa7d4ca752edff9a3205d", None, None),
    ("59d6b90306c377982e45c2941b935a5efddf436a", None, None),
    ("1617651e0a961bfa406f3deb58647906b34c02fc", None, None),
    ("91eba62fe07fbabfe2a0421ba57caa20e7e88db6", None, None),
    ("9405b41e8756c3652837b3f40d3075c6b7ee9bdc", None, None),
    ("bd237c2e3efbe690f61df1126b7e016a216f6cbd", None, None),
    ("ba371c8a76755c64379ed144ec55311775141d1e", None, None),
    ("14dcff357dd57410dab339c9b0db9a3d5dff8c0d", None, None),
    ("a8b6de0d27b384ee0f48154b2a3b93c0acc2b7e6", None, None),
    ("89b88cbe40e4d40b8c2351cd903ef127395ceae8", None, None),
    ("5bd713eb8ed1ea4d68010c1c4f7d2dcd2a95922b", None, None),
    ("3fcace64b931531fd44385d9b491909586033063", None, None),
    ("6f98a5ee336f98cb592838451c382b154499d0c4", None, None)
]

walk(path, branch=branch, commits=commits)

## Not enough disk space
## %%
### exonum
# from walkers.rustyrts_walker import walk
#
# path = "../projects/git_walk/exonum"
# branch = "master"
#
# commits = None #[("2d2fa22e5f5bc451d08c155c2398956f11dce06e", None, None)]
#
# walk(path, branch=branch, commits=commits)
#

# %%
## tantivy
from walkers.rustyrts_walker import walk

path = "../projects/git_walk/tantivy"
branch = "main"

commits = [
    ("f51801265640a8144655851072b6468e842f935c", None, None),
    ("c0f524e1a3f49f5609dfc34730959ac57feeaa49", None, None),
    ("2e639cebf89a185cd22e7b5aed25472b7a85b01b", None, None),
    ("36528c5e83e551a19f77e4e141fd09787c00d163", None, None),
    ("e2aa5af0759ff9077bb4490c7601c08d8e70c436", None, None),
    ("111f25a8f7c20abf90e2e6bc1cdd283dcab59b38", None, None),
    ("bc0eb813ffad75059a5e28ec46e2846371c75c06", None, None),
    ("bbb058d9769ca1bd8e4022488486de8410c39338", None, None),
    ("057211c3d8213ad0977e1d6e6f67e08f22cee332", None, None),
    ("bbc0a2e2331387b463067228e2cb6f6df6e78c4f", None, None),
    ("6bf4fee1baaec3d83b456d83d78f507f837a7bd8", None, None),
    ("4583fa270b24e812c37a22e7100a0c6d110f1d0a", None, None),
    ("70f160b3296e0dec38890e712f78f6d859a886f6", None, None),
    ("92f20bc5a23f004ec8d97c8a7bebeea408495ad6", None, None),
    ("784717749f5f129c71031391bb97234c408f677d", None, None),
    ("46d5de920dd1ac86fa7a74baa0debd933bcb6574", None, None),
    ("075c23eb8cc9d9fbfb3e47e2559189b66d980143", None, None),
    ("972cb6c26d002b2f83548855fe8cca0b98bf93d3", None, None),
    ("64bce340b2b32408be7c16cf77eddf5009542965", None, None),
    ("a550c85369f516edb8905e79f68b10637bf38894", None, None),
    ("762e662bfd2c7224a8e30e18fbe61f5b31ee9f42", None, None),
    ("6800fdec9db02a3bf8cdece5c98bfc0650bf415e", None, None),
    ("537021e12d38bcc3d2e5f33ee1d54ced66d057ed", None, None),
    ("46b86a797651e1d09dc9859ee4db705272e6cfa7", None, None),
    ("d31f04587258b47da704a945bbc41d10a759c465", None, None),
    ("62052bcc2d35364ce45ac837ea1b7cea2c106ea5", None, None),
    ("54decc60bb91077ad869eafa31bf3e6b1b22e655", None, None),
    ("cf02e3257836f6c6ae985db80ad5b03c84d147f0", None, None),
    ("443aa1732905264161b953c33cb944be2f5142e0", None, None),
    ("4d634d61ffecf238249cc556a39b6a88701ad6e3", None, None)
]

walk(path, branch=branch, commits=commits)

# %%
## meilisearch
from walkers.rustyrts_walker import walk

path = "../projects/git_walk/meilisearch"
branch = "main"

commits = None  # [("cb9d78fc7f23e77df7ce61f980ab91da7dc74233", None, None)]

walk(path, branch=branch, commits=commits)

# %%
## sled
from walkers.rustyrts_walker import walk

path = "../projects/git_walk/sled"
branch = "main"

commits = None  # [("69294e59c718289ab3cb6bd03ac3b9e1e072a1e7", "testing", "testing")]

walk(path, branch=branch, commits=commits)

## Not enough disk space
## %%
### penumbra
# from walkers.rustyrts_walker import walk
#
# path = "../projects/git_walk/penumbra"
# branch = "main"
#
# commits = None #[("3a946e1d1ae1fa16fe928a1bb14bc8b89e935e4f", None, None)]
#
# walk(path, branch=branch, commits=commits)
#

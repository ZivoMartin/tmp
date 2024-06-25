# Secret Sharing
    
Ce projet est un simulateur de systèmes de partage de secret. Pour le build, il faudra avoir Cargo installé ainsi que Gnuplot. Ensuite, exécutez `cargo build --release` (nécessairement avec l'option `--release`) pour le dépôt `nodes`, puis `cargo run` pour les dépôts `interface` qui permettent de lancer le serveur et `ui` qui permet de lancer l'interface utilisateur (UI) qui interagit proprement avec l'interface.

Pour lancer une simulation à travers l'UI, vous avez besoin d'un fichier de configuration placé dans le dossier `configs` à la racine du projet. Une fois le fichier construit, l'UI le reconnaîtra et l'affichera après actualisation (bouton en haut à gauche). Pour le lancer, il suffit de cliquer dessus.

### Comment configurer une simulation ?

Les fichiers de configuration suivent le format `.json`, en voici un exemple :

```json
[
    {
        "debit": 3,
        "output": "output_result",
        "hmt": 5,
        "verify": true,
        "reconstruct": true,
        "dealing": false,
        "first_receiv": true,
        "messages_computing": true,
        "total": true
    },
    {
        "n": [7, 11, "..", 15],
        "t": 3,
        "nb_byz": 0,
        "byz_comp": 1
    },
    {
        "n": 12,
        "t": 3,
        "nb_byz": [1, 2, 3],
        "byz_comp": 1
    }
]
```
        
On a ici un tableau de trois éléments. Intéressons-nous au premier, il s'agit des paramètres. Voici une description de chaque paramètre :

- **debit*: représente le nombre de secrets que le système peut absorber en `x` secondes (ce paramètre indique la valeur de `x`). 0 équivaut à ne pas calculer le débit, et son omission équivaut à 0.
- **output*: le nom du fichier dans lequel on souhaite rediriger l'output (nous reviendrons sur les fichiers de résultat plus tard). Son omission équivaut à ne pas vouloir observer de résultat.
- **hmt*: HMT (how many times) indique combien de fois l'on souhaite partager un secret dans chaque état.
- Tous les autres paramètres représentent des parties de l'algorithme. `true` veut dire que l'on souhaite une analyse de cette partie et `false` non. Son omission équivaut à `false`.


L'élément suivant du tableau est un partage, détaillons ses champs :

- **n*: Nombre de nœuds.
- **t*: Le seuil sera calculé comme suit : `(n-1)/t`.
- **nb_byz*: Le nombre de nœuds corrompus.
- **byz_comp*: Le comportement des nœuds corrompus. `0` pour agir normalement et `1` pour ne pas réagir lors de l'envoi d'un message.

Le champ `n` est sous la forme d'un tableau car c'est ce champ que l'on souhaite faire varier. Ainsi, nous allons lancer l'algorithme avec les valeurs `n = 7, 11, 12, 13, 14, 15`. Les `".."` sont du sucre syntaxique pour indiquer toutes les valeurs allant de 11 à 15. Chaque état sera ainsi lancé `hmt` fois.

L'élément suivant est simplement une deuxième simulation qui va faire varier le nombre de nœuds byzantins.
    
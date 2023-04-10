(function() {
	var button;
	Plugin.register('blockbyteexporter', {
	    title: 'BlockByte exporter',
	    author: 'mmm1245',
	    icon: 'icon',
	    description: 'Exports models to BlockByte format',
	    version: '1.0.0',
	    variant: 'both',
	    onload() {
	    	button = new Action('export_blockbyte_static', {
                name: 'Export as BlockByte static mesh',
                description: '',
                icon: 'bar_chart',
                click: function() {
                	let data = {};
                	let faces = [];
                	for(cube in Cube.all){
                		faces.push({"x1":cube.from.0, "y1":cube.from.1, "z1":cube.from.2, "x2":cube.from.0, "y2":cube.from.1, "z2":cube.from.2});
                	}
                    Blockbench.export({
                        type: 'BlockByte model',
                        extensions: ['json'],
                        name: (Project.name !== '' ? Project.name: "model"),
                        content: autoStringify(data),
                        savetype: 'json'
                    });
                }
            });
            MenuBar.addAction(button, 'file.export');
	    }
	});
})();


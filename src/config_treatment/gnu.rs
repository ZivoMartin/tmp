//use gtk::prelude::*;
//use gtk::{Application, ApplicationWindow, Image, Fixed};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::process::Command;
//use std::thread::sleep;
//use std::time::Duration;

pub fn plot_with_gnuplot(
    field: &str,
    x_data: &[u16],
    y_data: &[Vec<u16>],
    result_fields: &[&str],
    output_file: &str,
    base_state: &[(String, u16)],
    y_axe: &str,
) {
    let data_file_path = "data_temp.txt";
    {
        let data_file = File::create(data_file_path).unwrap();
        let mut data_writer = BufWriter::new(data_file);

        for (i, x) in x_data.iter().enumerate() {
            write!(data_writer, "{}", x).unwrap();
            for y in y_data {
                write!(data_writer, " {}", y[i]).unwrap();
            }
            writeln!(data_writer).unwrap();
        }
    }

    let script_file_path = "script_temp.gp";
    {
        let script_file = File::create(script_file_path).unwrap();
        let mut script_writer = BufWriter::new(script_file);
        let mut content = format!(
            "set terminal pngcairo\n\
             set output '{}'\n\
             set title 'Variation on {}'\n\
             set ylabel '{}'\n\
             set xlabel '{}'\n\
             ",
            output_file, field, y_axe, field
        );
        for (i, (f, v)) in base_state.iter().enumerate() {
            content.push_str(&format!(
                "set label '{f}: {v}' at graph 0.1, graph {} left font ',14'\n",
                0.9 - i as f32 * 0.1
            ))
        }
        content.push_str("plot ");
        for (i, result_field) in result_fields.iter().enumerate().take(y_data.len()) {
            if i > 0 {
                content.push_str(", ");
            }
            content.push_str(&format!(
                "'{}' using 1:{} with lines title '{}'",
                data_file_path,
                i + 2,
                result_field
            ));
        }
        write!(script_writer, "{}", content).unwrap();
    }

    let status = Command::new("gnuplot")
        .arg(script_file_path)
        .status()
        .unwrap();
    if !status.success() {
        panic!("Erreur lors de l'exécution de Gnuplot");
    }

    std::fs::remove_file(data_file_path).unwrap();
    std::fs::remove_file(script_file_path).unwrap();
}

// fn main() {
//     // Les données à tracer
//     let x_data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
//     let y_data = vec![2.0, 3.0, 5.0, 7.0, 11.0];
//     let output_file = "output.png";

//     // Générer le graphe avec Gnuplot
//     plot_with_gnuplot(&x_data, &y_data, output_file).expect("Erreur lors de la génération du graphe");

//     // Attendre un court instant pour s'assurer que le fichier est écrit sur le disque
//     sleep(Duration::from_millis(500));

//     // Vérifier que le fichier a été généré
//     if !std::path::Path::new(output_file).exists() {
//         eprintln!("Erreur: le fichier {} n'a pas été généré", output_file);
//         return;
//     } else {
//         println!("Fichier {} généré avec succès", output_file);
//     }

//     // Initialiser GTK
//     let app = Application::builder()
//         .application_id("com.example.gtk-gnuplot")
//         .build();

//     app.connect_activate(move |app| {
//         // Créer la fenêtre principale
//         let window = ApplicationWindow::builder()
//             .application(app)
//             .title("Graphe Gnuplot")
//             .default_width(800)
//             .default_height(600)
//             .build();

//         let fixed_container = Fixed::new();
//         // Charger l'image du graphe généré
//         match gtk::gdk_pixbuf::Pixbuf::from_file(output_file) {
//             Ok(pixbuf) => {
//                 // Créer un widget Image et y placer le pixbuf
//                 let image = Image::from_pixbuf(Some(&pixbuf));
//                 fixed_container.put(&image, 0, 0);
//             }
//             Err(err) => {
//                 eprintln!("Erreur lors du chargement de l'image: {}", err);
//             }
//         }
//         window.set_child(Some(&fixed_container));
//         // Afficher la fenêtre
//         window.show_all();
//     });

//     // Lancer l'application GTK
//     app.run();
// }
